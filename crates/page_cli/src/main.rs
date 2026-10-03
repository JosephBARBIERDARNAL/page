use std::collections::HashSet;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anstyle::{AnsiColor, Style};
use clap::builder::{PossibleValuesParser, TypedValueParser as _};
use clap::{Parser, ValueEnum};
use page_cli::output::{emit_json, serialize_json, write_atomic};
use page_cli::spinner::Spinner;
use page_validation::{
    JsonValidationReport, SafetyLimits, ValidationError, ValidationOptions, ValidationProfile,
    ValidationReport, validate_pdf, validate_pdf_lazy,
};

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum ColorArg {
    #[default]
    Auto,
    Never,
    Always,
}

#[derive(Debug, Parser)]
#[command(
    name = "page",
    bin_name = "page",
    version,
    about = "PDF/A and PDF/UA validation engine",
    group(clap::ArgGroup::new("safety_limits").multiple(true))
)]
struct Cli {
    /// PDF file to validate.
    file: PathBuf,

    /// Validation profile; defaults to the profile declared in XMP metadata.
    #[arg(long, value_name = "PROFILE", value_parser = profile_parser())]
    profile: Option<ValidationProfile>,

    /// Select detailed text or JSON output instead of the compact summary.
    #[arg(long, value_enum)]
    format: Option<FormatArg>,

    /// Write the report to a file; .json infers JSON when --format is omitted.
    #[arg(long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Colors in human-readable output.
    #[arg(long, value_enum, default_value_t)]
    color: ColorArg,

    /// Disable all configurable safety limits; use only with trusted files.
    #[arg(long, conflicts_with = "safety_limits")]
    disable_safety_limits: bool,

    /// Maximum input size in bytes.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_INPUT_SIZE)]
    max_input_size: u64,

    /// Maximum decoded size of any individual stream.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_DECODED_STREAM_SIZE)]
    max_decoded_stream_size: usize,

    /// Maximum combined decoded size of content streams and retained font streams.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE)]
    max_total_decoded_content_size: usize,

    /// Maximum number of Form XObject invocations across the document.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_FORM_INVOCATIONS)]
    max_form_invocations: usize,

    /// Maximum number of parsed indirect objects.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_OBJECT_COUNT)]
    max_object_count: usize,

    /// Maximum reference-chain depth used by the normalized model.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_REFERENCE_DEPTH)]
    max_reference_depth: usize,

    /// Maximum number of incremental-update revisions read from the cross-reference chain.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_XREF_REVISIONS)]
    max_xref_revisions: usize,

    /// Maximum number of rows or columns covered by one table cell.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_TABLE_SPAN)]
    max_table_span: usize,

    /// Maximum number of rows represented in an inspected table grid.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_TABLE_GRID_ROWS)]
    max_table_grid_rows: usize,

    /// Maximum number of columns represented in an inspected table grid.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_TABLE_GRID_COLUMNS)]
    max_table_grid_columns: usize,

    /// Maximum number of cells represented in an inspected table grid.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_TABLE_GRID_CELLS)]
    max_table_grid_cells: usize,

    /// Maximum number of mappings expanded from one ToUnicode CMap.
    #[arg(long, group = "safety_limits", default_value_t = SafetyLimits::DEFAULT_MAX_UNICODE_CMAP_MAPPINGS)]
    max_unicode_cmap_mappings: usize,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FormatArg {
    Details,
    Json,
}

fn profile_parser() -> impl clap::builder::TypedValueParser<Value = ValidationProfile> {
    PossibleValuesParser::new(
        ValidationProfile::all()
            .iter()
            .map(ValidationProfile::as_str),
    )
    .map(|profile| {
        profile
            .parse::<ValidationProfile>()
            .expect("available profile names must parse")
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectedFormat {
    Summary,
    Details,
    Json,
}

const FAILURE: Style = AnsiColor::Red.on_default().bold();
const SUMMARY_SUCCESS: Style = AnsiColor::BrightGreen.on_default().bold();
const SUMMARY_FAILURE: Style = AnsiColor::BrightRed.on_default().bold();

fn selected_style(enabled: bool, style: Style) -> Style {
    if enabled { style } else { Style::new() }
}

fn colors_enabled(color: ColorArg, no_color_env: bool, is_terminal: bool) -> bool {
    match color {
        ColorArg::Auto => !no_color_env && is_terminal,
        ColorArg::Always => true,
        ColorArg::Never => false,
    }
}

fn print_error(message: impl fmt::Display, colors: bool) {
    let error = selected_style(colors, FAILURE);
    eprintln!("{error}error:{error:#} {message}");
}

fn render_result_line(is_compliant: bool, colors: bool) -> String {
    let mut output = String::new();
    let (result_text, result_style) = if is_compliant {
        ("Conformant", SUMMARY_SUCCESS)
    } else {
        ("Non-conformant", SUMMARY_FAILURE)
    };
    let result = selected_style(colors, result_style);

    writeln!(output, "Result  : {result}{result_text}{result:#}")
        .expect("writing to a String cannot fail");
    output
}

fn render_summary(
    profile: ValidationProfile,
    is_compliant: bool,
    elapsed: Duration,
    colors: bool,
) -> String {
    let mut output = render_result_line(is_compliant, colors);
    writeln!(output, "Profile : {profile}").expect("writing to a String cannot fail");
    writeln!(output, "Time    : {:.3}s", elapsed.as_secs_f64())
        .expect("writing to a String cannot fail");
    output
}

fn render_details(report: &ValidationReport, elapsed: Duration, colors: bool) -> String {
    let mut output = render_result_line(report.is_compliant, colors);
    let failed_rules_style = selected_style(
        colors,
        if report.rules.failed == 0 {
            SUMMARY_SUCCESS
        } else {
            FAILURE
        },
    );
    let failed_checks_style = selected_style(
        colors,
        if report.checks.failed == 0 {
            SUMMARY_SUCCESS
        } else {
            FAILURE
        },
    );
    writeln!(
        output,
        "Rules   : {failed_rules_style}{}{failed_rules_style:#} failed {} / {} total",
        report.rules.failed,
        if report.rules.failed == 1 {
            "rule"
        } else {
            "rules"
        },
        report.rules.total,
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "Checks  : {failed_checks_style}{}{failed_checks_style:#} failed {}",
        report.checks.failed,
        if report.checks.failed == 1 {
            "check"
        } else {
            "checks"
        },
    )
    .expect("writing to a String cannot fail");
    writeln!(output, "Profile : {}", report.profile).expect("writing to a String cannot fail");
    writeln!(output, "Time    : {:.3}s", elapsed.as_secs_f64())
        .expect("writing to a String cannot fail");
    if !report.failures.is_empty() {
        output.push('\n');
    }
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
    let exit_code = error.exit_code();
    let report = JsonValidationReport::from_validation_error(
        Some(path.display().to_string()),
        profile,
        error,
    );
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

fn print_validation_error(path: &Path, error: &ValidationError, colors: bool) {
    match error {
        ValidationError::InputIo(error) => print_error(
            format_args!("could not read '{}': {error}", path.display()),
            colors,
        ),
        error => print_error(error, colors),
    }
}

fn main() {
    run_validate(Cli::parse());
}

fn run_validate(cli: Cli) {
    let no_color_env = std::env::var_os("NO_COLOR").is_some();
    let stdout_colors = colors_enabled(cli.color, no_color_env, io::stdout().is_terminal());
    let stderr_colors = colors_enabled(cli.color, no_color_env, io::stderr().is_terminal());
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
    let limits = if cli.disable_safety_limits {
        SafetyLimits::unlimited()
    } else {
        SafetyLimits::default()
            .max_input_size(cli.max_input_size)
            .max_decoded_stream_size(cli.max_decoded_stream_size)
            .max_total_decoded_content_size(cli.max_total_decoded_content_size)
            .max_form_invocations(cli.max_form_invocations)
            .max_object_count(cli.max_object_count)
            .max_reference_depth(cli.max_reference_depth)
            .max_xref_revisions(cli.max_xref_revisions)
            .max_table_span(cli.max_table_span)
            .max_table_grid_rows(cli.max_table_grid_rows)
            .max_table_grid_columns(cli.max_table_grid_columns)
            .max_table_grid_cells(cli.max_table_grid_cells)
            .max_unicode_cmap_mappings(cli.max_unicode_cmap_mappings)
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
    let requested_profile = cli.profile;
    let options = ValidationOptions::default()
        .profile(requested_profile)
        .limits(limits);
    if selected_format == SelectedFormat::Summary {
        let outcome = match validate_pdf_lazy(&cli.file, &options) {
            Ok(outcome) => outcome,
            Err(error) => {
                spinner.finish_and_clear();
                print_validation_error(&cli.file, &error, stderr_colors);
                std::process::exit(error.exit_code());
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
                    stdout_colors,
                )
            );
            if outcome.is_compliant { 0 } else { 2 }
        };
        std::process::exit(status);
    }
    let report = match validate_pdf(&cli.file, &options) {
        Ok(report) => report,
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
            print_validation_error(&cli.file, &error, stderr_colors);
            std::process::exit(error.exit_code());
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
                page_cli::validation_exit_code(&report)
            }
        }
        (None, SelectedFormat::Json) => {
            let json = report.json_report();
            match emit_json(&json, "validation report") {
                0 => page_cli::validation_exit_code(&report),
                status => status,
            }
        }
        (None, SelectedFormat::Details) => {
            print!("{}", render_details(&report, elapsed, stdout_colors));
            page_cli::validation_exit_code(&report)
        }
        (None, SelectedFormat::Summary) => page_cli::validation_exit_code(&report),
    };

    std::process::exit(status);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

    use super::{colors_enabled, render_details, render_summary};

    #[test]
    fn colors_require_a_terminal_and_no_opt_out() {
        assert!(colors_enabled(crate::ColorArg::Auto, false, true));
        assert!(!colors_enabled(crate::ColorArg::Never, false, true));
        assert!(!colors_enabled(crate::ColorArg::Auto, true, true));
        assert!(!colors_enabled(crate::ColorArg::Auto, false, false));
    }

    #[test]
    fn conformant_summary_uses_bright_green() {
        let summary = render_summary(ValidationProfile::PdfA1b, true, Duration::ZERO, true);
        assert!(summary.contains("92mConformant"));
    }

    #[test]
    fn detailed_counts_use_actual_plural_and_total_rules() {
        let bytes = include_bytes!("../../page_validation/tests/fixtures/structural.pdf");
        let mut report = validate_pdf_bytes(
            bytes,
            &ValidationOptions::default().profile(ValidationProfile::PdfA1b),
        )
        .expect("structural fixture should produce a report");
        report.rules.total = 10;
        report.rules.passed = 9;
        report.rules.failed = 1;
        report.checks.failed = 1;

        let details = render_details(&report, Duration::ZERO, false);

        assert!(details.contains("Rules   : 1 failed rule / 10 total"));
        assert!(details.contains("Checks  : 1 failed check"));
    }
}
