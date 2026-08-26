#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! clap = { version = "4.6", features = ["derive"] }
//! libc = "0.2"
//! ```

extern crate clap;
extern crate libc;

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;

//const BENCHMARK_PDF: &str = "bench/PDF32000_2008.pdf";
const BENCHMARK_PDF: &str = "../state-immunization-reports/documents/west_virginia.pdf";

#[derive(Debug, Parser)]
#[command(
    name = "verapdf-bench",
    about = "Compare end-to-end page and veraPDF validation performance"
)]
struct Cli {
    /// PDF file to validate with both validators.
    #[arg(long, default_value = BENCHMARK_PDF)]
    file: PathBuf,

    /// Path to the release page executable.
    #[arg(long, default_value = "target/release/page")]
    page: PathBuf,

    /// Path to the veraPDF executable.
    #[arg(long, default_value = "verapdf")]
    verapdf: PathBuf,

    /// Number of measured invocations for each validator.
    #[arg(long, default_value_t = 15)]
    runs: usize,

    /// Number of unmeasured invocations used to warm the filesystem cache.
    #[arg(long, default_value_t = 2)]
    warmup: usize,
}

#[derive(Clone, Copy, Debug)]
struct Summary {
    median: Duration,
    mean: Duration,
}

impl Summary {
    fn from_samples(mut samples: Vec<Duration>) -> Self {
        samples.sort_unstable();
        let total = samples.iter().copied().sum::<Duration>();
        let median = if samples.len().is_multiple_of(2) {
            (samples[samples.len() / 2 - 1] + samples[samples.len() / 2]) / 2
        } else {
            samples[samples.len() / 2]
        };
        Self {
            median,
            mean: total / samples.len() as u32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct MemSummary {
    median: u64,
    mean: u64,
}

#[derive(Clone, Copy, Debug)]
struct RunSample {
    elapsed: Duration,
    peak_rss: Option<u64>,
    exit_code: i32,
}

impl MemSummary {
    fn from_samples(mut samples: Vec<u64>) -> Self {
        samples.sort_unstable();
        let total: u64 = samples.iter().sum();
        let median = if samples.len().is_multiple_of(2) {
            (samples[samples.len() / 2 - 1] + samples[samples.len() / 2]) / 2
        } else {
            samples[samples.len() / 2]
        };
        Self {
            median,
            mean: total / samples.len() as u64,
        }
    }
}

/// Runs `command`, waiting on the child directly with `wait4` so the returned
/// peak RSS belongs to this one process. Polling `getrusage(RUSAGE_CHILDREN)`
/// instead would report a running historical max across every child reaped so
/// far, which sticks at veraPDF's much larger footprint once it has run once.
#[cfg(unix)]
fn spawn_and_wait(command: &mut Command) -> io::Result<RunSample> {
    let started = Instant::now();
    let child = command.spawn()?;
    let pid = child.id() as libc::pid_t;
    std::mem::forget(child);

    let mut status: libc::c_int = 0;
    let mut rusage: libc::rusage = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::wait4(pid, &mut status, 0, &mut rusage) };
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    if status & 0x7f != 0 {
        return Err(io::Error::other(format!(
            "benchmark process terminated by signal {}",
            status & 0x7f
        )));
    }
    let elapsed = started.elapsed();
    Ok(RunSample {
        elapsed,
        peak_rss: Some(peak_rss_bytes(rusage.ru_maxrss)),
        exit_code: status >> 8,
    })
}

#[cfg(not(unix))]
fn spawn_and_wait(command: &mut Command) -> io::Result<RunSample> {
    let started = Instant::now();
    let status = command.status()?;
    let exit_code = status
        .code()
        .ok_or_else(|| io::Error::other("benchmark process terminated by signal"))?;
    Ok(RunSample {
        elapsed: started.elapsed(),
        peak_rss: None,
        exit_code,
    })
}

#[cfg(target_os = "macos")]
fn peak_rss_bytes(maxrss: libc::c_long) -> u64 {
    maxrss as u64
}

#[cfg(all(unix, not(target_os = "macos")))]
fn peak_rss_bytes(maxrss: libc::c_long) -> u64 {
    maxrss as u64 * 1024
}

fn ensure_expected_exit(
    validator: &str,
    sample: RunSample,
    expected: &[i32],
) -> io::Result<RunSample> {
    if expected.contains(&sample.exit_code) {
        Ok(sample)
    } else {
        Err(io::Error::other(format!(
            "{validator} exited with {}; expected one of {expected:?}",
            sample.exit_code
        )))
    }
}

fn run(executable: &Path, args: &[&Path], expected: &[i32]) -> io::Result<RunSample> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    ensure_expected_exit(
        executable.to_string_lossy().as_ref(),
        spawn_and_wait(&mut command)?,
        expected,
    )
}

fn run_page(executable: &Path, file: &Path) -> io::Result<RunSample> {
    let mut command = Command::new(executable);
    command
        .arg("validate")
        .arg(file)
        .args([
            "--profile",
            "ua-1",
            "--format",
            "json",
            "--max-reference-depth",
            "512",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    ensure_expected_exit("page", spawn_and_wait(&mut command)?, &[0, 2])
}

fn run_verapdf(executable: &Path, file: &Path) -> io::Result<RunSample> {
    run(
        executable,
        &[
            Path::new("--loglevel"),
            Path::new("0"),
            Path::new("--disableerrormessages"),
            Path::new("--format"),
            Path::new("json"),
            Path::new("--flavour"),
            Path::new("ua1"),
            file,
        ],
        &[0, 1],
    )
}

fn format_ms(duration: Duration) -> String {
    format!("{:.3} ms", duration.as_secs_f64() * 1_000.0)
}

fn format_mb(bytes: u64) -> String {
    format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
}

fn print_summary(label: &str, summary: Summary) {
    println!(
        "  {label:8} median {:>12} mean {:>12}",
        format_ms(summary.median),
        format_ms(summary.mean),
    );
}

fn print_mem_summary(label: &str, summary: MemSummary) {
    println!(
        "  {label:8} median {:>12} mean {:>12}",
        format_mb(summary.median),
        format_mb(summary.mean),
    );
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    if cli.runs == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--runs must be greater than zero",
        ));
    }
    let file = cli.file.as_path();
    if !file.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("benchmark PDF not found: {}", file.display()),
        ));
    }

    println!(
        "\n\nPDF: {} with {} measured runs, {} warmup run\n",
        file.display(),
        cli.runs,
        cli.warmup
    );

    for _ in 0..cli.warmup {
        run_page(&cli.page, file)?;
        run_verapdf(&cli.verapdf, file)?;
    }

    let mut page_time = Vec::with_capacity(cli.runs);
    let mut page_mem = Vec::with_capacity(cli.runs);
    let mut verapdf_time = Vec::with_capacity(cli.runs);
    let mut verapdf_mem = Vec::with_capacity(cli.runs);
    for run_number in 0..cli.runs {
        let (page_sample, verapdf_sample) = if run_number.is_multiple_of(2) {
            let page_sample = run_page(&cli.page, file)?;
            let verapdf_sample = run_verapdf(&cli.verapdf, file)?;
            (page_sample, verapdf_sample)
        } else {
            let verapdf_sample = run_verapdf(&cli.verapdf, file)?;
            let page_sample = run_page(&cli.page, file)?;
            (page_sample, verapdf_sample)
        };
        page_time.push(page_sample.elapsed);
        page_mem.push(page_sample.peak_rss);
        verapdf_time.push(verapdf_sample.elapsed);
        verapdf_mem.push(verapdf_sample.peak_rss);
    }

    let page = Summary::from_samples(page_time);
    let verapdf = Summary::from_samples(verapdf_time);
    println!("time:");
    print_summary("page", page);
    print_summary("veraPDF", verapdf);
    println!(
        "speedup: {:.2}x (veraPDF median / page median)",
        verapdf.median.as_secs_f64() / page.median.as_secs_f64()
    );

    let page_mem: Option<Vec<u64>> = page_mem.into_iter().collect();
    let verapdf_mem: Option<Vec<u64>> = verapdf_mem.into_iter().collect();
    match (page_mem, verapdf_mem) {
        (Some(page_mem), Some(verapdf_mem)) => {
            let page_mem = MemSummary::from_samples(page_mem);
            let verapdf_mem = MemSummary::from_samples(verapdf_mem);
            println!("\npeak RSS:");
            print_mem_summary("page", page_mem);
            print_mem_summary("veraPDF", verapdf_mem);
            println!(
                "ratio: {:.2}x (veraPDF median RSS / page median RSS)",
                verapdf_mem.median as f64 / page_mem.median as f64
            );
        }
        _ => println!("\npeak RSS: n/a (not supported on this platform)"),
    }
    Ok(())
}
