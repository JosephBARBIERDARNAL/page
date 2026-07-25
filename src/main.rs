use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use pdf::{SafetyLimits, ValidationProfile, validate_file};

#[derive(Debug, Parser)]
#[command(
    name = "pdf",
    version,
    about = "Preliminary PDF/A validator (not a complete conformance checker)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the preliminary validation rules.
    Validate {
        /// The preliminary validation profile.
        #[arg(long, value_enum)]
        profile: ProfileArg,

        /// Report format.
        #[arg(long, value_enum, default_value_t = ReportFormat::Text)]
        format: ReportFormat,

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

        /// PDF file to validate.
        file: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProfileArg {
    #[value(name = "pdfa-1b")]
    PdfA1b,
}

impl From<ProfileArg> for ValidationProfile {
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::PdfA1b => Self::PdfA1b,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum ReportFormat {
    #[default]
    Text,
    Json,
}

fn main() {
    let cli = Cli::parse();
    let status = match cli.command {
        Command::Validate {
            profile,
            format,
            max_input_size,
            max_decoded_stream_size,
            max_object_count,
            max_reference_depth,
            file,
        } => {
            let limits = SafetyLimits {
                max_input_size,
                max_decoded_stream_size,
                max_object_count,
                max_reference_depth,
            };
            let report = validate_file(&file, profile.into(), &limits);
            match format {
                ReportFormat::Text => print!("{report}"),
                ReportFormat::Json => match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(error) => {
                        eprintln!("could not serialize validation report: {error}");
                        std::process::exit(1);
                    }
                },
            }
            report.exit_code()
        }
    };

    std::process::exit(status);
}
