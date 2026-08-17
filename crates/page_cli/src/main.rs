use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anstyle::{AnsiColor, Style};
use clap::{Args, Parser, Subcommand, ValueEnum};
use page_cli::output::{emit_json, serialize_json, write_atomic};
use page_validation::{
    FailureCategory, SafetyLimits, ValidationError, ValidationProfile, ValidationReport,
    validate_file, validate_file_with_profile,
};

mod corpus;

#[derive(Debug, Parser)]
#[command(
    name = "page",
    bin_name = "page",
    version,
    about = "Experimental PDF/A and PDF/UA validator"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a PDF document.
    Validate(ValidateArgs),

    /// Validate the selected profiles from a veraPDF corpus checkout.
    Corpus(corpus::CorpusArgs),
}

#[derive(Debug, Args)]
struct ValidateArgs {
    /// PDF file to validate.
    file: PathBuf,

    /// Validation profile; defaults to the profile declared in XMP metadata.
    #[arg(long, value_enum)]
    profile: Option<ProfileArg>,

    /// Select detailed text or JSON output instead of the compact summary.
    #[arg(long, value_enum)]
    format: Option<FormatArg>,

    /// Write the report to a file; .json infers JSON when --format is omitted.
    #[arg(long, value_name = "FILE")]
    output: Option<PathBuf>,

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectedFormat {
    Summary,
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

const FAILURE: Style = AnsiColor::Red.on_default().bold();
const WARNING: Style = AnsiColor::Yellow.on_default().bold();
const SUMMARY_SUCCESS: Style = AnsiColor::BrightGreen.on_default().bold();
const SUMMARY_FAILURE: Style = AnsiColor::BrightRed.on_default().bold();

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SummaryCategory {
    PdfSyntax,
    Profile,
    Metadata,
    Color,
    Fonts,
    Images,
    Graphics,
    InteractiveContent,
    Structure,
}

const SUMMARY_CATEGORIES: [SummaryCategory; 9] = [
    SummaryCategory::PdfSyntax,
    SummaryCategory::Profile,
    SummaryCategory::Metadata,
    SummaryCategory::Color,
    SummaryCategory::Fonts,
    SummaryCategory::Images,
    SummaryCategory::Graphics,
    SummaryCategory::InteractiveContent,
    SummaryCategory::Structure,
];

fn summary_category(failure: &page_validation::ValidationFailure) -> SummaryCategory {
    let rule = &failure.rule_id;

    if failure.category == FailureCategory::Parser
        || failure.category == FailureCategory::Operational
        || rule.starts_with("PDFA1B-HEADER-")
        || rule.starts_with("PDFA1B-POST-EOF-")
        || rule.starts_with("PDFA1B-XREF-")
        || rule.starts_with("PDFA1B-INDIRECT-OBJECT-SYNTAX-")
        || rule.starts_with("PDFA1B-HEX-STRING-")
        || rule.starts_with("PDFA1B-STREAM-EOL-")
        || rule.starts_with("PDFA1B-STREAM-LENGTH-")
    {
        SummaryCategory::PdfSyntax
    } else if failure.category == FailureCategory::Metadata || rule.starts_with("PDFA1B-INFO-") {
        SummaryCategory::Metadata
    } else if rule.contains("-OUTPUTINTENT-")
        || rule.contains("-ICCBASED-")
        || rule.contains("-DEVICE-")
        || rule.contains("-DEVICEN-")
    {
        SummaryCategory::Color
    } else if rule.contains("-FONT-")
        || rule.contains("-TRUETYPE-")
        || rule.contains("-TYPE0-")
        || rule.contains("-TYPE1-")
        || rule.contains("-CID-")
        || rule.contains("-CIDTOGIDMAP-")
        || rule.contains("-CMAP-")
    {
        SummaryCategory::Fonts
    } else if rule.contains("-IMAGE-") {
        SummaryCategory::Images
    } else if rule.contains("-CONTENT-OPERATOR-")
        || rule.contains("-EXTGSTATE-")
        || rule.contains("-GRAPHICS-")
        || rule.contains("-RENDERING-INTENT-")
        || rule.contains("-TRANSPARENCY-")
        || rule.contains("-XOBJECT-")
    {
        SummaryCategory::Graphics
    } else if rule.contains("-ANNOTATION-")
        || rule.contains("-ACTION-")
        || rule.contains("-ACROFORM-")
        || rule.contains("-FIELD-")
        || rule.contains("-FORM-")
        || rule.contains("-WIDGET-")
    {
        SummaryCategory::InteractiveContent
    } else if rule.contains("-CATALOG-")
        || rule.contains("-FILE-SPEC-")
        || rule.contains("-NAMES-EMBEDDED-FILES-")
        || rule.contains("-OPTIONAL-CONTENT-")
        || rule.contains("-TRAILER-ID-")
        || rule.contains("-STREAM-EXTERNAL-DATA-")
        || rule.contains("-STREAM-LZW-")
    {
        SummaryCategory::Structure
    } else {
        SummaryCategory::Profile
    }
}

fn category_label(category: SummaryCategory, report: &ValidationReport) -> String {
    match category {
        SummaryCategory::PdfSyntax => "PDF syntax".to_owned(),
        SummaryCategory::Profile => report.profile.to_string(),
        SummaryCategory::Metadata => "Metadata".to_owned(),
        SummaryCategory::Color => "Color".to_owned(),
        SummaryCategory::Fonts => "Fonts".to_owned(),
        SummaryCategory::Images => "Images".to_owned(),
        SummaryCategory::Graphics => "Graphics".to_owned(),
        SummaryCategory::InteractiveContent => "Interactive content".to_owned(),
        SummaryCategory::Structure => "Structure".to_owned(),
    }
}

fn render_summary(report: &ValidationReport, elapsed: Duration, colors: bool) -> String {
    let mut output = String::new();
    let (result_text, result_style) = if report.has_operational_failure() {
        ("Incomplete", WARNING)
    } else if report
        .failures
        .iter()
        .any(|failure| failure.category == FailureCategory::Parser)
    {
        ("Invalid PDF", SUMMARY_FAILURE)
    } else if report.checks_passed {
        ("Conformant", SUMMARY_SUCCESS)
    } else {
        ("Non-conformant", SUMMARY_FAILURE)
    };
    let result = selected_style(colors, result_style);

    writeln!(output, "Profile : {}", report.profile).expect("writing to a String cannot fail");
    writeln!(output, "Result  : {result}{result_text}{result:#}")
        .expect("writing to a String cannot fail");
    output.push('\n');

    for category in SUMMARY_CATEGORIES {
        let failed = report
            .failures
            .iter()
            .any(|failure| summary_category(failure) == category);
        let (symbol, style) = if failed {
            ('✗', selected_style(colors, SUMMARY_FAILURE))
        } else {
            ('✓', selected_style(colors, SUMMARY_SUCCESS))
        };
        writeln!(
            output,
            "{style}{symbol}{style:#} {}",
            category_label(category, report)
        )
        .expect("writing to a String cannot fail");
    }

    writeln!(output, "\nTime    : {:.3}s", elapsed.as_secs_f64())
        .expect("writing to a String cannot fail");
    output
}

fn render_details(report: &ValidationReport, elapsed: Duration, colors: bool) -> String {
    let mut output = render_summary(report, elapsed, colors);
    output.push('\n');
    writeln!(
        output,
        "Checks: {} passed, {} failed, {} total",
        report.checks.passed, report.checks.failed, report.checks.total
    )
    .expect("writing to a String cannot fail");
    if let Some(document) = &report.document {
        writeln!(
            output,
            "Document: PDF {}, {} page(s), {} object(s)",
            document.version, document.page_count, document.object_count
        )
        .expect("writing to a String cannot fail");
    }
    let rule = selected_style(colors, FAILURE);
    for failure in &report.failures {
        write!(
            output,
            "{rule}[{}]{rule:#} {:?}: {}",
            failure.rule_id, failure.category, failure.message
        )
        .expect("writing to a String cannot fail");
        if let Some(id) = failure.object_id {
            write!(output, " (object {} {})", id.object_number, id.generation)
                .expect("writing to a String cannot fail");
        }
        output.push('\n');
    }
    output
}

fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|extension| extension.to_str())
}

fn select_format(
    format: Option<FormatArg>,
    output: Option<&Path>,
) -> Result<SelectedFormat, String> {
    let extension = output.and_then(extension);
    match format {
        Some(FormatArg::Json)
            if extension.is_some_and(|value| value.eq_ignore_ascii_case("txt")) =>
        {
            Err("output extension '.txt' conflicts with JSON format".to_owned())
        }
        Some(FormatArg::Details)
            if extension.is_some_and(|value| value.eq_ignore_ascii_case("json")) =>
        {
            Err("output extension '.json' conflicts with details format".to_owned())
        }
        Some(FormatArg::Json) => Ok(SelectedFormat::Json),
        Some(FormatArg::Details) => Ok(SelectedFormat::Details),
        None if extension.is_some_and(|value| value.eq_ignore_ascii_case("json")) => {
            Ok(SelectedFormat::Json)
        }
        None => Ok(SelectedFormat::Summary),
    }
}

fn paths_refer_to_same_file(input: &Path, output: &Path) -> bool {
    if input == output {
        return true;
    }

    if let (Ok(input), Ok(output)) = (fs::canonicalize(input), fs::canonicalize(output))
        && input == output
    {
        return true;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        if let (Ok(input), Ok(output)) = (fs::metadata(input), fs::metadata(output)) {
            return input.dev() == output.dev() && input.ino() == output.ino();
        }
    }

    false
}

fn main() {
    match Cli::parse().command {
        Command::Validate(cli) => run_validate(cli),
        Command::Corpus(args) => std::process::exit(corpus::run(&args)),
    }
}

fn run_validate(cli: ValidateArgs) {
    let no_color_env = std::env::var_os("NO_COLOR").is_some();
    let stdout_colors = colors_enabled(cli.no_color, no_color_env, io::stdout().is_terminal());
    let stderr_colors = colors_enabled(cli.no_color, no_color_env, io::stderr().is_terminal());
    let selected_format = match select_format(cli.format, cli.output.as_deref()) {
        Ok(format) => format,
        Err(error) => {
            print_error(error, stderr_colors);
            std::process::exit(1);
        }
    };
    if let Some(output) = &cli.output
        && paths_refer_to_same_file(&cli.file, output)
    {
        print_error(
            "input and output paths refer to the same file",
            stderr_colors,
        );
        std::process::exit(1);
    }
    let limits = SafetyLimits {
        max_input_size: cli.max_input_size,
        max_decoded_stream_size: cli.max_decoded_stream_size,
        max_object_count: cli.max_object_count,
        max_reference_depth: cli.max_reference_depth,
    };
    let started_at = Instant::now();
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
    let elapsed = started_at.elapsed();
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
        match (cli.output.as_deref(), selected_format) {
            (Some(output), format) => {
                let rendered = match format {
                    SelectedFormat::Summary => Ok(render_summary(&report, elapsed, false)),
                    SelectedFormat::Details => Ok(render_details(&report, elapsed, false)),
                    SelectedFormat::Json => {
                        let json = report.json_report();
                        serialize_json(&json).map_err(|error| {
                            format!("could not serialize validation report: {error}")
                        })
                    }
                };
                let rendered = match rendered {
                    Ok(rendered) => rendered,
                    Err(error) => {
                        print_error(error, stderr_colors);
                        std::process::exit(1);
                    }
                };
                if let Err(error) = write_atomic(output, rendered.as_bytes()) {
                    print_error(
                        format_args!("could not write '{}': {error}", output.display()),
                        stderr_colors,
                    );
                    1
                } else {
                    report.exit_code()
                }
            }
            (None, SelectedFormat::Json) => {
                let json = report.json_report();
                match emit_json(&json, "validation report") {
                    0 => report.exit_code(),
                    status => status,
                }
            }
            (None, SelectedFormat::Details) => {
                print!("{}", render_details(&report, elapsed, stdout_colors));
                report.exit_code()
            }
            (None, SelectedFormat::Summary) => {
                print!("{}", render_summary(&report, elapsed, stdout_colors));
                report.exit_code()
            }
        }
    };

    std::process::exit(status);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use page_validation::{ValidationCounts, ValidationProfile, ValidationReport};

    use super::{colors_enabled, render_summary};

    #[test]
    fn colors_require_a_terminal_and_no_opt_out() {
        assert!(colors_enabled(false, false, true));
        assert!(!colors_enabled(true, false, true));
        assert!(!colors_enabled(false, true, true));
        assert!(!colors_enabled(false, false, false));
    }

    #[test]
    fn conformant_summary_uses_bright_green() {
        let report = ValidationReport {
            source: None,
            profile: ValidationProfile::PdfA1b,
            checks_passed: true,
            preliminary: true,
            checks: ValidationCounts {
                total: 1,
                passed: 1,
                failed: 0,
            },
            document: None,
            failures: Vec::new(),
        };

        let summary = render_summary(&report, Duration::ZERO, true);

        assert!(summary.contains("92mConformant"));
    }
}
