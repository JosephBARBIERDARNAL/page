use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use tag_cli::output::{JsonValidationReport, emit_json};
use tag_validation::{SafetyLimits, ValidationProfile, validate_file};

#[derive(Debug, Parser)]
#[command(
    name = "tag",
    bin_name = "tag",
    version,
    about = "Experimental PDF/A validator"
)]
struct Cli {
    /// PDF file to validate.
    file: PathBuf,

    /// Validation profile.
    #[arg(long, value_enum)]
    profile: ProfileArg,

    /// Emit the validation report as JSON.
    #[arg(long)]
    json: bool,

    /// Maximum input size in bytes.
    #[arg(long, default_value_t = SafetyLimits::DEFAULT_MAX_INPUT_SIZE)]
    max_input_size: u64,

    /// Maximum decoded size of any individual stream.
    #[arg(long, default_value_t = SafetyLimits::DEFAULT_MAX_DECODED_STREAM_SIZE)]
    max_decoded_stream_size: usize,

    /// Maximum number of parsed indirect objects.
    #[arg(long, default_value_t = SafetyLimits::DEFAULT_MAX_OBJECT_COUNT)]
    max_object_count: usize,

    /// Maximum reference-chain depth used by the normalized model.
    #[arg(long, default_value_t = SafetyLimits::DEFAULT_MAX_REFERENCE_DEPTH)]
    max_reference_depth: usize,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProfileArg {
    #[value(name = "a-1b")]
    PdfA1b,
}

impl From<ProfileArg> for ValidationProfile {
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::PdfA1b => Self::PdfA1b,
        }
    }
}

impl ProfileArg {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PdfA1b => "a-1b",
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let limits = SafetyLimits {
        max_input_size: cli.max_input_size,
        max_decoded_stream_size: cli.max_decoded_stream_size,
        max_object_count: cli.max_object_count,
        max_reference_depth: cli.max_reference_depth,
    };
    let profile = cli.profile;
    let report = validate_file(&cli.file, profile.into(), &limits);
    let status = if cli.json {
        let json = JsonValidationReport::from_report(
            cli.file.display().to_string(),
            profile.as_str(),
            &report,
        );
        match emit_json(&json, "validation report") {
            0 => report.exit_code(),
            status => status,
        }
    } else if let Some(failure) = report
        .failures
        .iter()
        .find(|failure| failure.rule_id == "INPUT-IO-001")
    {
        eprintln!(
            "error: could not read '{}': {}",
            cli.file.display(),
            failure.message
        );
        report.exit_code()
    } else {
        print!("{report}");
        report.exit_code()
    };

    std::process::exit(status);
}
