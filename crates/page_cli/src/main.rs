use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use page_cli::output::{JsonValidationReport, emit_json};
use page_validation::{SafetyLimits, ValidationProfile, validate_file};

#[derive(Debug, Parser)]
#[command(
    name = "page",
    bin_name = "page",
    version,
    about = "Experimental PDF/A and PDF/UA validator"
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
    #[value(name = "a-1a")]
    PdfA1a,
    #[value(name = "a-2b")]
    PdfA2b,
    #[value(name = "a-2a")]
    PdfA2a,
    #[value(name = "a-2u")]
    PdfA2u,
    #[value(name = "a-3b")]
    PdfA3b,
    #[value(name = "a-3a")]
    PdfA3a,
    #[value(name = "a-3u")]
    PdfA3u,
    #[value(name = "a-4")]
    PdfA4,
    #[value(name = "a-4e")]
    PdfA4e,
    #[value(name = "a-4f")]
    PdfA4f,
    #[value(name = "ua-1")]
    PdfUa1,
    #[value(name = "ua-2")]
    PdfUa2,
}

impl From<ProfileArg> for ValidationProfile {
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::PdfA1b => Self::PdfA1b,
            ProfileArg::PdfA1a => Self::PdfA1a,
            ProfileArg::PdfA2b => Self::PdfA2b,
            ProfileArg::PdfA2a => Self::PdfA2a,
            ProfileArg::PdfA2u => Self::PdfA2u,
            ProfileArg::PdfA3b => Self::PdfA3b,
            ProfileArg::PdfA3a => Self::PdfA3a,
            ProfileArg::PdfA3u => Self::PdfA3u,
            ProfileArg::PdfA4 => Self::PdfA4,
            ProfileArg::PdfA4e => Self::PdfA4e,
            ProfileArg::PdfA4f => Self::PdfA4f,
            ProfileArg::PdfUa1 => Self::PdfUa1,
            ProfileArg::PdfUa2 => Self::PdfUa2,
        }
    }
}

impl ProfileArg {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PdfA1b => "a-1b",
            Self::PdfA1a => "a-1a",
            Self::PdfA2b => "a-2b",
            Self::PdfA2a => "a-2a",
            Self::PdfA2u => "a-2u",
            Self::PdfA3b => "a-3b",
            Self::PdfA3a => "a-3a",
            Self::PdfA3u => "a-3u",
            Self::PdfA4 => "a-4",
            Self::PdfA4e => "a-4e",
            Self::PdfA4f => "a-4f",
            Self::PdfUa1 => "ua-1",
            Self::PdfUa2 => "ua-2",
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let profile = cli.profile;
    let validation_profile: ValidationProfile = profile.into();
    if !validation_profile.is_implemented() {
        eprintln!("error: validation profile {validation_profile} is not implemented yet");
        std::process::exit(1);
    }

    let limits = SafetyLimits {
        max_input_size: cli.max_input_size,
        max_decoded_stream_size: cli.max_decoded_stream_size,
        max_object_count: cli.max_object_count,
        max_reference_depth: cli.max_reference_depth,
    };
    let report = validate_file(&cli.file, validation_profile, &limits);
    let status = if let Some(failure) = report
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
    } else if cli.json {
        let json = JsonValidationReport::from_report(
            cli.file.display().to_string(),
            profile.as_str(),
            &report,
        );
        match emit_json(&json, "validation report") {
            0 => report.exit_code(),
            status => status,
        }
    } else {
        print!("{report}");
        report.exit_code()
    };

    std::process::exit(status);
}
