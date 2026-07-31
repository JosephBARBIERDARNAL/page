use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "verapdf-bench",
    bin_name = "verapdf-bench",
    version,
    about = "Compare end-to-end tag and veraPDF validation performance"
)]
struct Cli {
    /// Path to the release tag executable.
    #[arg(long, default_value = "target/release/tag")]
    tag: PathBuf,

    /// Path to the veraPDF executable.
    #[arg(long)]
    verapdf: PathBuf,

    /// Number of measured invocations for each validator and file.
    #[arg(long, default_value_t = 10)]
    runs: usize,

    /// Number of unmeasured invocations used to warm the filesystem cache.
    #[arg(long, default_value_t = 1)]
    warmup: usize,

    /// PDFs to validate, measured one at a time by both executables.
    #[arg(required = true, num_args = 1..)]
    files: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug)]
struct Summary {
    min: Duration,
    median: Duration,
    p95: Duration,
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
        let p95_index = (samples.len() * 95).div_ceil(100).saturating_sub(1);
        let p95 = samples[p95_index];
        Self {
            min: samples[0],
            median,
            p95,
            mean: total / samples.len() as u32,
        }
    }
}

fn run_validator(executable: &Path, args: &[&Path]) -> io::Result<Duration> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let started = Instant::now();
    command.status()?;
    Ok(started.elapsed())
}

fn run_tag(executable: &Path, file: &Path) -> io::Result<Duration> {
    let mut command = Command::new(executable);
    command
        .arg(file)
        .args(["--profile", "a-1b", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let started = Instant::now();
    command.status()?;
    Ok(started.elapsed())
}

fn run_verapdf(executable: &Path, file: &Path) -> io::Result<Duration> {
    run_validator(
        executable,
        &[
            Path::new("--loglevel"),
            Path::new("0"),
            Path::new("--format"),
            Path::new("json"),
            Path::new("--flavour"),
            Path::new("1b"),
            file,
        ],
    )
}

fn format_ms(duration: Duration) -> String {
    format!("{:.3} ms", duration.as_secs_f64() * 1_000.0)
}

fn print_summary(label: &str, summary: Summary) {
    println!(
        "  {label:8} median {:>12} mean {:>12} min {:>12} p95 {:>12}",
        format_ms(summary.median),
        format_ms(summary.mean),
        format_ms(summary.min),
        format_ms(summary.p95),
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
    println!(
        "end-to-end benchmark: {} measured run(s), {} warmup run(s)",
        cli.runs, cli.warmup
    );
    println!("tag: {}", cli.tag.display());
    println!("veraPDF: {}", cli.verapdf.display());

    for file in &cli.files {
        for _ in 0..cli.warmup {
            run_tag(&cli.tag, file)?;
            run_verapdf(&cli.verapdf, file)?;
        }

        let mut tag_samples = Vec::with_capacity(cli.runs);
        let mut verapdf_samples = Vec::with_capacity(cli.runs);
        for run in 0..cli.runs {
            if run % 2 == 0 {
                tag_samples.push(run_tag(&cli.tag, file)?);
                verapdf_samples.push(run_verapdf(&cli.verapdf, file)?);
            } else {
                verapdf_samples.push(run_verapdf(&cli.verapdf, file)?);
                tag_samples.push(run_tag(&cli.tag, file)?);
            }
        }

        let tag = Summary::from_samples(tag_samples);
        let verapdf = Summary::from_samples(verapdf_samples);
        println!("\n{}", file.display());
        print_summary("tag", tag);
        print_summary("veraPDF", verapdf);
        println!(
            "  speedup: {:.2}x (veraPDF median / tag median)",
            verapdf.median.as_secs_f64() / tag.median.as_secs_f64()
        );
    }
    Ok(())
}
