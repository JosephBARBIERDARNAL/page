use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use pdf_cli::output::{ReportFormat, emit_json};
use pdf_validation::SafetyLimits;
use pdf_validation::differential::{
    DEFAULT_MAX_REPORT_BYTES, DEFAULT_TIMEOUT_MILLIS, DifferentialRunner, PINNED_VERAPDF_VERSION,
    ReferenceConfig, ReferenceProfile, aggregate_exit_code,
};

#[derive(Debug, Parser)]
#[command(
    name = "verapdf-diff",
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

    /// Maximum veraPDF execution time per command.
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_MILLIS)]
    timeout_millis: u64,

    /// Maximum captured veraPDF JSON size.
    #[arg(long, default_value_t = DEFAULT_MAX_REPORT_BYTES)]
    max_reference_report_bytes: usize,

    /// One or more explicit PDF paths.
    #[arg(required = true, num_args = 1..)]
    files: Vec<PathBuf>,
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

    let runner = match DifferentialRunner::new(config) {
        Ok(runner) => runner,
        Err(failure) => {
            match cli.format {
                ReportFormat::Text => {
                    eprintln!(
                        "Operational reference failure ({:?}): {}",
                        failure.kind, failure.message
                    );
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
    let reports = cli
        .files
        .iter()
        .map(|file| runner.compare_file(file, &limits))
        .collect::<Vec<_>>();
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
