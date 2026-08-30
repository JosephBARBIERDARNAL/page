#!/usr/bin/env rust-script

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

const RUNS: usize = 10;
const WARMUP_RUNS: usize = 2;
const BENCHMARK_DIRECTORY: &str = "bench";
const OUTPUT_PATH: &str = "docs/benchmark.md";
const PAGE_EXECUTABLE: &str = "target/release/page";
const VERAPDF_EXECUTABLE: &str = "verapdf";

#[derive(Clone, Copy, Debug)]
struct RunSample {
    elapsed_seconds: f64,
    exit_code: i32,
}

#[derive(Clone, Copy, Debug)]
struct Summary {
    mean_seconds: f64,
}

impl Summary {
    fn from_samples(samples: &[RunSample]) -> Self {
        let mean_seconds = samples
            .iter()
            .map(|sample| sample.elapsed_seconds)
            .sum::<f64>()
            / samples.len() as f64;
        Self { mean_seconds }
    }
}

#[derive(Debug)]
struct BenchmarkResult {
    document: String,
    verapdf: Summary,
    page_fail_fast: Summary,
    page_normal: Summary,
}

fn spawn_and_wait(command: &mut Command) -> io::Result<RunSample> {
    let started = Instant::now();
    let status = command.status()?;
    let exit_code = status
        .code()
        .ok_or_else(|| io::Error::other("benchmark process terminated by signal"))?;
    Ok(RunSample {
        elapsed_seconds: started.elapsed().as_secs_f64(),
        exit_code,
    })
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

fn run_page(executable: &Path, file: &Path, fail_fast: bool) -> io::Result<RunSample> {
    let mut command = Command::new(executable);
    command
        .arg(file)
        .args(["--profile", "ua1", "--max-reference-depth", "512"]);
    if !fail_fast {
        command.args(["--format", "details"]);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    ensure_expected_exit(
        if fail_fast {
            "page (fail fast)"
        } else {
            "page (normal)"
        },
        spawn_and_wait(&mut command)?,
        &[0, 2],
    )
}

fn run_verapdf(executable: &Path, file: &Path) -> io::Result<RunSample> {
    let mut command = Command::new(executable);
    command
        .args([
            "--loglevel",
            "0",
            "--disableerrormessages",
            "--format",
            "json",
            "--flavour",
            "ua1",
        ])
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    ensure_expected_exit("veraPDF", spawn_and_wait(&mut command)?, &[0, 1])
}

fn benchmark_files(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = fs::read_dir(directory)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("document") && name.ends_with(".pdf"))
        })
        .collect::<Vec<_>>();
    files.sort();
    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no document*.pdf files found in {}", directory.display()),
        ));
    }
    Ok(files)
}

fn format_speedup(reference_seconds: f64, seconds: f64) -> String {
    format!("{:.1}×", reference_seconds / seconds)
}

fn markdown(results: &[BenchmarkResult]) -> String {
    let mut output = format!(
        "Mean validation speedups are based on timings measured in milliseconds and normalized per document to veraPDF = 1.0×. Higher values are faster. Each value uses {RUNS} runs (with {WARMUP_RUNS} warmup runs) per validator and document.\n\n"
    );
    output.push_str("The 5 documents have varying size, content and method of creation. We're working on making the benchmark fully reproducible and share the documents used.\n\n");
    output.push_str("| Document | veraPDF | page | page fail-fast |\n");
    output.push_str("| --- | ---: | ---: | ---: |\n");
    for result in results {
        output.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            result.document,
            format_speedup(result.verapdf.mean_seconds, result.verapdf.mean_seconds,),
            format_speedup(result.verapdf.mean_seconds, result.page_normal.mean_seconds),
            format_speedup(
                result.verapdf.mean_seconds,
                result.page_fail_fast.mean_seconds,
            ),
        ));
    }
    output.push_str("\n!!! note\n\n");
    output.push_str("      The fail fast mode of `page` (used automatically when possible) allows to get much faster results, but does not give details about which specific rules failed.");
    output
}

fn main() -> io::Result<()> {
    let files = benchmark_files(Path::new(BENCHMARK_DIRECTORY))?;
    println!(
        "Benchmarking {} documents with {} measured runs and {} warmup runs each",
        files.len(),
        RUNS,
        WARMUP_RUNS
    );

    let mut results = Vec::with_capacity(files.len());
    for file in files {
        let document = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.display().to_string());
        println!("  {document}");

        for _ in 0..WARMUP_RUNS {
            run_verapdf(Path::new(VERAPDF_EXECUTABLE), &file)?;
            run_page(Path::new(PAGE_EXECUTABLE), &file, true)?;
            run_page(Path::new(PAGE_EXECUTABLE), &file, false)?;
        }

        let mut verapdf_samples = Vec::with_capacity(RUNS);
        let mut page_fail_fast_samples = Vec::with_capacity(RUNS);
        let mut page_normal_samples = Vec::with_capacity(RUNS);
        for run_number in 0..RUNS {
            for offset in 0..3 {
                match (run_number + offset) % 3 {
                    0 => verapdf_samples.push(run_verapdf(Path::new(VERAPDF_EXECUTABLE), &file)?),
                    1 => page_fail_fast_samples.push(run_page(
                        Path::new(PAGE_EXECUTABLE),
                        &file,
                        true,
                    )?),
                    _ => page_normal_samples.push(run_page(
                        Path::new(PAGE_EXECUTABLE),
                        &file,
                        false,
                    )?),
                }
            }
        }

        results.push(BenchmarkResult {
            document,
            verapdf: Summary::from_samples(&verapdf_samples),
            page_fail_fast: Summary::from_samples(&page_fail_fast_samples),
            page_normal: Summary::from_samples(&page_normal_samples),
        });
    }

    fs::write(OUTPUT_PATH, markdown(&results))?;
    println!("Wrote {OUTPUT_PATH}");
    Ok(())
}
