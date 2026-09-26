#!/usr/bin/env rust-script

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

const RUNS: usize = 10;
const WARMUP_RUNS: usize = 2;
const BENCHMARK_DIRECTORY: &str = "bench";
const OUTPUT_PATH: &str = "docs/benchmark.md";
const PAGE_EXECUTABLE: &str = "target/release/page";
const VERAPDF_EXECUTABLE: &str = "verapdf";
const PROFILES: [&str; 3] = ["1b", "2b", "ua1"];

#[derive(Clone, Copy, Debug)]
struct RunSample {
    elapsed_seconds: f64,
    exit_code: i32,
}

#[derive(Clone, Copy, Debug)]
struct Summary {
    median_seconds: f64,
}

impl Summary {
    fn from_samples(samples: &[RunSample]) -> io::Result<Self> {
        if samples.is_empty() {
            return Err(io::Error::other("benchmark produced no samples"));
        }
        let mut values = samples
            .iter()
            .map(|sample| sample.elapsed_seconds)
            .collect::<Vec<_>>();
        values.sort_by(f64::total_cmp);
        Ok(Self {
            median_seconds: quantile(&values, 0.5)?,
        })
    }
}

#[derive(Debug)]
struct ProfileBenchmark {
    profile: &'static str,
    verapdf: Summary,
    page_fail_fast: Summary,
    page_normal: Summary,
}

#[derive(Debug)]
struct DocumentBenchmark {
    document: String,
    size_bytes: u64,
    page_count: usize,
    profiles: Vec<ProfileBenchmark>,
}

fn progress(message: impl std::fmt::Display) -> io::Result<()> {
    println!("{message}");
    io::stdout().flush()
}

fn quantile(sorted_values: &[f64], fraction: f64) -> io::Result<f64> {
    let max_index = sorted_values
        .len()
        .checked_sub(1)
        .ok_or_else(|| io::Error::other("benchmark produced no samples"))?;
    let position = fraction * max_index as f64;
    let lower_index = position.floor() as usize;
    let upper_index = position.ceil() as usize;
    let lower = *sorted_values
        .get(lower_index)
        .ok_or_else(|| io::Error::other("invalid benchmark quantile"))?;
    let upper = *sorted_values
        .get(upper_index)
        .ok_or_else(|| io::Error::other("invalid benchmark quantile"))?;
    Ok(lower + (upper - lower) * position.fract())
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

fn run_page(
    executable: &Path,
    file: &Path,
    profile: &str,
    fail_fast: bool,
) -> io::Result<RunSample> {
    let mut command = Command::new(executable);
    command
        .arg(file)
        .args(["--profile", profile, "--max-reference-depth", "512"]);
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

fn run_verapdf(executable: &Path, file: &Path, profile: &str) -> io::Result<RunSample> {
    let mut command = Command::new(executable);
    command
        .args([
            "--loglevel",
            "0",
            "--disableerrormessages",
            "--format",
            "json",
            "--flavour",
            profile,
        ])
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    ensure_expected_exit("veraPDF", spawn_and_wait(&mut command)?, &[0, 1])
}

fn benchmark_profile(file: &Path, profile: &'static str) -> io::Result<ProfileBenchmark> {
    for warmup in 0..WARMUP_RUNS {
        progress(format!("      warmup {}/{}", warmup + 1, WARMUP_RUNS))?;
        run_page(Path::new(PAGE_EXECUTABLE), file, profile, false)?;
        run_page(Path::new(PAGE_EXECUTABLE), file, profile, true)?;
        run_verapdf(Path::new(VERAPDF_EXECUTABLE), file, profile)?;
    }

    let mut verapdf_samples = Vec::with_capacity(RUNS);
    let mut page_fail_fast_samples = Vec::with_capacity(RUNS);
    let mut page_normal_samples = Vec::with_capacity(RUNS);
    for run_number in 0..RUNS {
        progress(format!("      measured run {}/{}", run_number + 1, RUNS))?;
        for offset in 0..3 {
            match (run_number + offset) % 3 {
                0 => {
                    verapdf_samples.push(run_verapdf(Path::new(VERAPDF_EXECUTABLE), file, profile)?)
                }
                1 => page_fail_fast_samples.push(run_page(
                    Path::new(PAGE_EXECUTABLE),
                    file,
                    profile,
                    true,
                )?),
                _ => page_normal_samples.push(run_page(
                    Path::new(PAGE_EXECUTABLE),
                    file,
                    profile,
                    false,
                )?),
            }
        }
    }

    let result = ProfileBenchmark {
        profile,
        verapdf: Summary::from_samples(&verapdf_samples)?,
        page_fail_fast: Summary::from_samples(&page_fail_fast_samples)?,
        page_normal: Summary::from_samples(&page_normal_samples)?,
    };
    progress(format!(
        "      complete: page {}, page FF {} versus veraPDF",
        format_speedup(result.verapdf, result.page_normal),
        format_speedup(result.verapdf, result.page_fail_fast),
    ))?;
    Ok(result)
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

fn format_mib(size_bytes: u64) -> String {
    format!("{:.1}", size_bytes as f64 / (1024.0 * 1024.0))
}

fn format_speedup(reference: Summary, candidate: Summary) -> String {
    format!(
        "{:.2}×",
        reference.median_seconds / candidate.median_seconds
    )
}

fn document_label(file: &Path) -> String {
    let stem = file
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("document");
    if let Some(number) = stem.strip_prefix("document") {
        if number.chars().all(|character| character.is_ascii_digit()) && !number.is_empty() {
            return format!("document {number}");
        }
    }
    stem.strip_prefix("document-").unwrap_or(stem).to_owned()
}

fn profile<'a>(
    benchmark: &'a DocumentBenchmark,
    profile_name: &str,
) -> io::Result<&'a ProfileBenchmark> {
    benchmark
        .profiles
        .iter()
        .find(|result| result.profile == profile_name)
        .ok_or_else(|| io::Error::other(format!("missing {profile_name} benchmark result")))
}

fn profile_label(profile_name: &str) -> &str {
    match profile_name {
        "1b" => "PDF/A-1b",
        "2b" => "PDF/A-2b",
        "ua1" => "PDF/UA-1",
        _ => profile_name,
    }
}

fn markdown(results: &[DocumentBenchmark]) -> io::Result<String> {
    let mut output = format!(
        "Each result is a relative speedup versus veraPDF for the same profile; higher is faster. Values use the median of {RUNS} measured runs with {WARMUP_RUNS} warmups.\n\n"
    );
    output.push_str("!!! note \"What is page FF?\"\n\n    `page FF` is page's fail-fast mode: it stops after the first failure and reports only whether the document is compliant. `page` runs the exhaustive path and collects all implemented failures.\n\n");
    output.push_str("The corpus includes the existing documents and a deterministic feature-heavy PDF with multiple pages, images, an embedded font, structure elements, optional-content groups, a name tree, and embedded files.\n\n");
    for document in results {
        output.push_str(&format!("## {}\n\n", document.document));
        output.push_str(&format!(
            "- Size: {} MiB\n- Pages: {}\n",
            format_mib(document.size_bytes),
            document.page_count,
        ));
        for profile_name in PROFILES {
            let result = profile(document, profile_name)?;
            output.push_str(&format!(
                "- {}: `page` **{}**, `page FF` **{}** versus veraPDF\n",
                profile_label(profile_name),
                format_speedup(result.verapdf, result.page_normal),
                format_speedup(result.verapdf, result.page_fail_fast),
            ));
        }
        output.push('\n');
    }

    output.push_str("The feature-heavy case is intentionally non-compliant; it makes profile-demand differences measurable rather than representing a normal publishing workload.\n");
    Ok(output)
}

fn page_count(file: &Path) -> io::Result<usize> {
    let output = Command::new("pdfinfo").arg(file).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "pdfinfo failed for {}",
            file.display()
        )));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Pages:")?.trim().parse().ok())
        .ok_or_else(|| {
            io::Error::other(format!(
                "pdfinfo did not report pages for {}",
                file.display()
            ))
        })
}

fn main() -> io::Result<()> {
    let files = benchmark_files(Path::new(BENCHMARK_DIRECTORY))?;
    progress(format!(
        "Benchmarking {} documents across {} profiles with {} measured runs and {} warmup runs each",
        files.len(),
        PROFILES.len(),
        RUNS,
        WARMUP_RUNS
    ))?;

    let mut results = Vec::with_capacity(files.len());
    let document_count = files.len();
    for (document_index, file) in files.into_iter().enumerate() {
        let document = document_label(&file);
        let size_bytes = fs::metadata(&file)?.len();
        let page_count = page_count(&file)?;
        progress(format!(
            "  document {}/{}: {} ({:.1} MiB, {} pages)",
            document_index + 1,
            document_count,
            document,
            size_bytes as f64 / (1024.0 * 1024.0),
            page_count,
        ))?;
        let mut profiles = Vec::with_capacity(PROFILES.len());
        for (profile_index, profile_name) in PROFILES.into_iter().enumerate() {
            progress(format!(
                "    profile {}/{}: {profile_name}",
                profile_index + 1,
                PROFILES.len(),
            ))?;
            profiles.push(benchmark_profile(&file, profile_name)?);
        }
        results.push(DocumentBenchmark {
            document,
            size_bytes,
            page_count,
            profiles,
        });
    }

    fs::write(OUTPUT_PATH, markdown(&results)?)?;
    progress(format!("Wrote {OUTPUT_PATH}"))?;
    Ok(())
}
