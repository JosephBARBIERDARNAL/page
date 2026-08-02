use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use page_cli::output::{ReportFormat, emit_json};
use page_validation::SafetyLimits;
use page_validation::differential::{
    CoverageGapPolicy, DEFAULT_BATCH_SIZE, DEFAULT_MAX_REPORT_BYTES, DEFAULT_TIMEOUT_MILLIS,
    DifferentialRunner, PINNED_VERAPDF_VERSION, ReferenceConfig, ReferenceProfile,
    aggregate_exit_code,
};

#[derive(Debug, Parser)]
#[command(
    name = "verapdf-diff",
    bin_name = "verapdf-diff",
    version,
    about = "Compare preliminary local PDF/A-1b checks with pinned veraPDF"
)]
struct Cli {
    /// Path to the veraPDF executable.
    #[arg(long)]
    verapdf: PathBuf,

    /// Required veraPDF version.
    #[arg(long, default_value = PINNED_VERAPDF_VERSION)]
    expected_version: String,

    /// Explicit veraPDF flavour.
    #[arg(long, value_enum, default_value_t = ProfileArg::PdfA1b)]
    profile: ProfileArg,

    /// Output format.
    #[arg(long, value_enum, default_value_t = ReportFormat::Text)]
    format: ReportFormat,

    /// Maximum veraPDF execution time per batched command.
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_MILLIS)]
    timeout_millis: u64,

    /// Maximum captured veraPDF JSON size.
    #[arg(long, default_value_t = DEFAULT_MAX_REPORT_BYTES)]
    max_reference_report_bytes: usize,

    /// Maximum number of PDFs passed to one veraPDF invocation.
    #[arg(long, default_value_t = NonZeroUsize::new(DEFAULT_BATCH_SIZE).unwrap())]
    batch_size: NonZeroUsize,

    /// One or more explicit PDF paths.
    #[arg(required = true, num_args = 1..)]
    files: Vec<PathBuf>,

    /// Reject coverage_gap, as required after PDF/A-1B completion is declared.
    #[arg(long)]
    require_complete: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProfileArg {
    #[value(name = "1b")]
    PdfA1b,
}

impl From<ProfileArg> for ReferenceProfile {
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::PdfA1b => Self::PdfA1b,
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let mut config = ReferenceConfig::pinned(cli.verapdf);
    config.expected_version = cli.expected_version;
    config.profile = cli.profile.into();
    config.timeout_millis = cli.timeout_millis;
    config.max_report_bytes = cli.max_reference_report_bytes;
    config.batch_size = cli.batch_size.get();
    if cli.require_complete {
        config.coverage_gap_policy = CoverageGapPolicy::RejectForCompleteProfile;
    }

    let runner = match DifferentialRunner::new(config) {
        Ok(runner) => runner,
        Err(failure) => {
            match cli.format {
                ReportFormat::Text => {
                    eprintln!("{failure}");
                }
                ReportFormat::Json => match serde_json::to_string_pretty(&failure) {
                    Ok(json) => eprintln!("{json}"),
                    Err(error) => eprintln!("could not serialize operational failure: {error}"),
                },
            }
            std::process::exit(1);
        }
    };

    let limits = SafetyLimits::default();
    let reports = runner.compare_files(&cli.files, &limits);
    match cli.format {
        ReportFormat::Text => {
            for (index, report) in reports.iter().enumerate() {
                if index > 0 {
                    println!();
                }
                print!("{report}");
            }
        }
        ReportFormat::Json => {
            let status = emit_json(&reports, "differential reports");
            if status != 0 {
                std::process::exit(status);
            }
        }
    }
    std::process::exit(aggregate_exit_code(&reports));
}
