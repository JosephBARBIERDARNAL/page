use std::collections::HashSet;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anstyle::{AnsiColor, Style};
use clap::{Parser, ValueEnum};
use page_cli::output::{emit_json, serialize_json, write_atomic};
use page_cli::spinner::Spinner;
use page_validation::{
    JsonError, JsonErrorKind, JsonValidationReport, SafetyLimits, ValidationError, ValidationInput,
    ValidationProfile, ValidationReport, is_pdf_compliant, validate_file,
};

#[derive(Debug, Parser)]
#[command(
    name = "page",
    bin_name = "page",
    version,
    about = "PDF/A and PDF/UA validaton engine"
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

    /// Maximum total decoded size of page, Form, appearance, Pattern, and Type3 content streams.
    #[arg(long, default_value_t = SafetyLimits::DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE)]
    max_total_decoded_content_size: usize,

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
    #[value(name = "1b")]
    PdfA1b,
    #[value(name = "1a")]
    PdfA1a,
    #[value(name = "2b")]
    PdfA2b,
    #[value(name = "2a")]
    PdfA2a,
    #[value(name = "2u")]
    PdfA2u,
    #[value(name = "3b")]
    PdfA3b,
    #[value(name = "3a")]
    PdfA3a,
    #[value(name = "3u")]
    PdfA3u,
    #[value(name = "4")]
    PdfA4,
    #[value(name = "4e")]
    PdfA4e,
    #[value(name = "4f")]
    PdfA4f,
    #[value(name = "ua1")]
    PdfUa1,
    #[value(name = "ua2")]
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

fn render_summary(
    profile: ValidationProfile,
    is_compliant: bool,
    elapsed: Duration,
    colors: bool,
) -> String {
    let mut output = String::new();
    let (result_text, result_style) = if is_compliant {
        ("Conformant", SUMMARY_SUCCESS)
    } else {
        ("Non-conformant", SUMMARY_FAILURE)
    };
    let result = selected_style(colors, result_style);

    writeln!(output, "Profile : {profile}").expect("writing to a String cannot fail");
    writeln!(output, "Result  : {result}{result_text}{result:#}")
        .expect("writing to a String cannot fail");
    writeln!(output, "Time    : {:.3}s", elapsed.as_secs_f64())
        .expect("writing to a String cannot fail");
    output
}

fn render_details(report: &ValidationReport, elapsed: Duration, colors: bool) -> String {
    let mut output = render_summary(report.profile, report.checks_passed, elapsed, colors);
    output.push('\n');
    let mut seen = HashSet::new();
    let rule = selected_style(colors, FAILURE);
    for failure in &report.failures {
        if !seen.insert((
            &failure.rule_id,
            failure.category as u8,
            &failure.message,
            failure
                .object_id
                .map(|object_id| (object_id.object_number, object_id.generation)),
        )) {
            continue;
        }
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

fn emit_json_validation_error(
    path: &Path,
    profile: Option<ValidationProfile>,
    error: ValidationError,
    output: Option<&Path>,
    colors: bool,
) -> ! {
    let (kind, rule, exit_code) = match &error {
        ValidationError::Pdf(_) => (JsonErrorKind::Parser, "PDF-PARSE-001", 2),
        _ => (JsonErrorKind::Operational, "VALIDATION-PROFILE-001", 1),
    };
    let report = JsonValidationReport {
        file: Some(path.display().to_string()),
        profile,
        valid: false,
        failures: Vec::new(),
        error: Some(JsonError {
            kind,
            rule: rule.to_owned(),
            message: error.to_string(),
        }),
    };
    if let Some(output) = output {
        let contents = serialize_json(&report).unwrap_or_else(|serialization_error| {
            print_error(
                format_args!("could not serialize validation report: {serialization_error}"),
                colors,
            );
            std::process::exit(1);
        });
        if let Err(write_error) = write_atomic(output, contents.as_bytes()) {
            print_error(
                format_args!("could not write '{}': {write_error}", output.display()),
                colors,
            );
            std::process::exit(1);
        }
        std::process::exit(exit_code);
    }
    std::process::exit(if emit_json(&report, "validation report") == 0 {
        exit_code
    } else {
        1
    });
}

fn main() {
    run_validate(Cli::parse());
}

fn run_validate(cli: Cli) {
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
        max_total_decoded_content_size: cli.max_total_decoded_content_size,
        max_object_count: cli.max_object_count,
        max_reference_depth: cli.max_reference_depth,
        max_xref_revisions: SafetyLimits::DEFAULT_MAX_XREF_REVISIONS,
    };
    let spinner_enabled = selected_format != SelectedFormat::Json
        && io::stdout().is_terminal()
        && io::stderr().is_terminal();
    let started_at = Instant::now();
    let spinner = Spinner::new(
        spinner_enabled,
        stderr_colors,
        format!("Validating {}", cli.file.display()),
    );
    let requested_profile = cli.profile.map(Into::into);
    if selected_format == SelectedFormat::Summary {
        let outcome =
            match is_pdf_compliant(ValidationInput::File(&cli.file), requested_profile, &limits) {
                Ok(outcome) => outcome,
                Err(ValidationError::InputIo(error)) => {
                    spinner.finish_and_clear();
                    print_error(
                        format_args!("could not read '{}': {error}", cli.file.display()),
                        stderr_colors,
                    );
                    std::process::exit(1);
                }
                Err(error) => {
                    spinner.finish_and_clear();
                    print_error(error, stderr_colors);
                    std::process::exit(1);
                }
            };
        spinner.finish_and_clear();
        let elapsed = started_at.elapsed();
        let rendered = render_summary(outcome.profile, outcome.is_compliant, elapsed, false);
        let status = if let Some(output) = cli.output.as_deref() {
            if let Err(error) = write_atomic(output, rendered.as_bytes()) {
                print_error(
                    format_args!("could not write '{}': {error}", output.display()),
                    stderr_colors,
                );
                1
            } else if outcome.is_compliant {
                0
            } else {
                2
            }
        } else {
            print!(
                "{}",
                render_summary(
                    outcome.profile,
                    outcome.is_compliant,
                    elapsed,
                    stdout_colors
                )
            );
            if outcome.is_compliant { 0 } else { 2 }
        };
        std::process::exit(status);
    }
    let report = match validate_file(&cli.file, requested_profile, &limits) {
        Ok(report) => report,
        Err(ValidationError::InputIo(error)) => {
            spinner.finish_and_clear();
            print_error(
                format_args!("could not read '{}': {error}", cli.file.display()),
                stderr_colors,
            );
            std::process::exit(1);
        }
        Err(error) => {
            spinner.finish_and_clear();
            if selected_format == SelectedFormat::Json {
                emit_json_validation_error(
                    &cli.file,
                    requested_profile,
                    error,
                    cli.output.as_deref(),
                    stderr_colors,
                );
            }
            print_error(error, stderr_colors);
            std::process::exit(1);
        }
    };
    spinner.finish_and_clear();
    let elapsed = started_at.elapsed();
    let status = match (cli.output.as_deref(), selected_format) {
        (Some(output), format) => {
            let rendered = match format {
                SelectedFormat::Summary => Ok(String::new()),
                SelectedFormat::Details => Ok(render_details(&report, elapsed, false)),
                SelectedFormat::Json => {
                    let json = report.json_report();
                    serialize_json(&json)
                        .map_err(|error| format!("could not serialize validation report: {error}"))
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
        (None, SelectedFormat::Summary) => report.exit_code(),
    };

    std::process::exit(status);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use page_validation::ValidationProfile;

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
        let summary = render_summary(ValidationProfile::PdfA1b, true, Duration::ZERO, true);

        assert!(summary.contains("92mConformant"));
    }
}
