use std::fmt;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

use anstyle::{AnsiColor, Style};
use clap::{Parser, ValueEnum};
use page_cli::output::emit_json;
use page_validation::{
    FailureCategory, SafetyLimits, ValidationError, ValidationProfile, ValidationReport,
    validate_file, validate_file_with_profile,
};

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

    /// Validation profile; defaults to the profile declared in XMP metadata.
    #[arg(long, value_enum)]
    profile: Option<ProfileArg>,

    /// Select detailed text or JSON output instead of the compact summary.
    #[arg(long, value_enum)]
    format: Option<FormatArg>,

    /// Disable colors in human-readable output.
    #[arg(long)]
    no_color: bool,

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
enum FormatArg {
    Details,
    Json,
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

const EMPHASIS: Style = Style::new().bold();
const SUCCESS: Style = AnsiColor::Green.on_default().bold();
const FAILURE: Style = AnsiColor::Red.on_default().bold();
const WARNING: Style = AnsiColor::Yellow.on_default().bold();

fn selected_style(enabled: bool, style: Style) -> Style {
    if enabled { style } else { Style::new() }
}

fn colors_enabled(no_color_flag: bool, no_color_env: bool, is_terminal: bool) -> bool {
    !no_color_flag && !no_color_env && is_terminal
}

fn print_error(message: impl fmt::Display, colors: bool) {
    let error = selected_style(colors, FAILURE);
    eprintln!("{error}error:{error:#} {message}");
}

fn print_summary(report: &ValidationReport, colors: bool) {
    let profile = selected_style(colors, EMPHASIS);
    if report.has_operational_failure() {
        let result = selected_style(colors, WARNING);
        println!(
            "{profile}{}{profile:#}: {result}validation could not be completed{result:#}",
            report.profile
        );
    } else if report
        .failures
        .iter()
        .any(|failure| failure.category == FailureCategory::Parser)
    {
        let result = selected_style(colors, FAILURE);
        println!(
            "{profile}{}{profile:#}: {result}input could not be parsed{result:#}",
            report.profile
        );
    } else if report.checks_passed {
        let result = selected_style(colors, SUCCESS);
        println!(
            "{profile}{}{profile:#}: {result}all {} implemented checks passed{result:#}",
            report.profile, report.checks.total
        );
    } else {
        let result = selected_style(colors, FAILURE);
        println!(
            "{profile}{}{profile:#}: {result}{}/{} implemented checks failed{result:#}",
            report.profile, report.checks.failed, report.checks.total
        );
    }
}

fn print_details(report: &ValidationReport, colors: bool) {
    let heading = selected_style(colors, EMPHASIS);
    let result = selected_style(
        colors,
        if report.checks_passed {
            SUCCESS
        } else {
            FAILURE
        },
    );
    println!("{heading}Preliminary PDF/A validation{heading:#}");
    println!("Profile: {}", report.profile);
    println!(
        "Result: {result}{}{result:#}",
        if report.checks_passed {
            "no failures in implemented checks"
        } else {
            "failed"
        }
    );
    println!(
        "Checks: {} passed, {} failed, {} total",
        report.checks.passed, report.checks.failed, report.checks.total
    );
    if let Some(document) = &report.document {
        println!(
            "Document: PDF {}, {} page(s), {} object(s)",
            document.version, document.page_count, document.object_count
        );
    }
    let rule = selected_style(colors, FAILURE);
    for failure in &report.failures {
        print!(
            "{rule}[{}]{rule:#} {:?}: {}",
            failure.rule_id, failure.category, failure.message
        );
        if let Some(id) = failure.object_id {
            print!(" (object {} {})", id.object_number, id.generation);
        }
        println!();
    }
}

fn main() {
    let cli = Cli::parse();
    let no_color_env = std::env::var_os("NO_COLOR").is_some();
    let stdout_colors = colors_enabled(cli.no_color, no_color_env, io::stdout().is_terminal());
    let stderr_colors = colors_enabled(cli.no_color, no_color_env, io::stderr().is_terminal());
    let limits = SafetyLimits {
        max_input_size: cli.max_input_size,
        max_decoded_stream_size: cli.max_decoded_stream_size,
        max_object_count: cli.max_object_count,
        max_reference_depth: cli.max_reference_depth,
    };
    let report = match cli.profile {
        Some(profile) => {
            let validation_profile: ValidationProfile = profile.into();
            if !validation_profile.is_implemented() {
                print_error(
                    format_args!("validation profile {validation_profile} is not implemented yet"),
                    stderr_colors,
                );
                std::process::exit(1);
            }
            validate_file_with_profile(&cli.file, validation_profile, &limits)
        }
        None => match validate_file(&cli.file, &limits) {
            Ok(report) => report,
            Err(ValidationError::InputIo(error)) => {
                print_error(
                    format_args!("could not read '{}': {error}", cli.file.display()),
                    stderr_colors,
                );
                std::process::exit(1);
            }
            Err(error) => {
                print_error(error, stderr_colors);
                std::process::exit(1);
            }
        },
    };
    let status = if let Some(failure) = report
        .failures
        .iter()
        .find(|failure| failure.rule_id == "INPUT-IO-001")
    {
        print_error(
            format_args!(
                "could not read '{}': {}",
                cli.file.display(),
                failure.message
            ),
            stderr_colors,
        );
        report.exit_code()
    } else {
        match cli.format {
            Some(FormatArg::Json) => {
                let json = report.json_report(cli.file.display().to_string());
                match emit_json(&json, "validation report") {
                    0 => report.exit_code(),
                    status => status,
                }
            }
            Some(FormatArg::Details) => {
                print_details(&report, stdout_colors);
                report.exit_code()
            }
            None => {
                print_summary(&report, stdout_colors);
                report.exit_code()
            }
        }
    };

    std::process::exit(status);
}

#[cfg(test)]
mod tests {
    use super::colors_enabled;

    #[test]
    fn colors_require_a_terminal_and_no_opt_out() {
        assert!(colors_enabled(false, false, true));
        assert!(!colors_enabled(true, false, true));
        assert!(!colors_enabled(false, true, true));
        assert!(!colors_enabled(false, false, false));
    }
}
