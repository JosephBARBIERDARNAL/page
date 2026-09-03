use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::IsTerminal;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Args;
use page_cli::spinner::Spinner;
use page_validation::{SafetyLimits, ValidationProfile, ValidationReport, validate_pdf};

#[derive(Debug, Args)]
pub(crate) struct CorpusArgs {
    /// Root directory of a veraPDF corpus checkout.
    #[arg(value_name = "DIRECTORY")]
    pub(crate) directory: PathBuf,

    /// Maximum number of PDFs to validate concurrently; defaults to at most four workers.
    #[arg(long, value_name = "JOBS")]
    pub(crate) jobs: Option<NonZeroUsize>,

    /// Optional rule expectation manifest; defaults to the pinned veraPDF manifest bundled with page.
    #[arg(long, value_name = "FILE")]
    pub(crate) rule_manifest: Option<PathBuf>,
}

#[derive(Clone, Copy)]
struct CorpusProfile {
    directory: &'static str,
    profile: ValidationProfile,
}

#[derive(Clone, Copy)]
enum CorpusSourceProfile {
    Fixed(ValidationProfile),
    Iso32000,
    Twg,
}

#[derive(Clone, Copy)]
struct CorpusSource {
    directory: &'static str,
    manifest_directory: &'static str,
    profile: CorpusSourceProfile,
}

const CORPUS_PROFILE_SPEC: &str = include_str!("corpus_profiles.txt");
const CORPUS_RULE_EXPECTATIONS_SPEC: &str = include_str!("corpus_rule_expectations.tsv");
const CORPUS_RESULT_EXPECTATIONS_SPEC: &str = include_str!("corpus_result_expectations.tsv");

fn corpus_profiles() -> Result<Vec<CorpusProfile>, String> {
    CORPUS_PROFILE_SPEC
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut fields = line.split_whitespace();
            let directory = fields
                .next()
                .ok_or_else(|| "corpus profile entry has no directory".to_owned())?;
            let profile = fields
                .next()
                .ok_or_else(|| format!("corpus profile entry '{directory}' has no profile"))?;
            if fields.next().is_some() {
                return Err(format!(
                    "corpus profile entry '{directory}' has too many fields"
                ));
            }
            let profile = match profile {
                "a-1a" => ValidationProfile::PdfA1a,
                "a-1b" => ValidationProfile::PdfA1b,
                "a-2a" => ValidationProfile::PdfA2a,
                "a-2b" => ValidationProfile::PdfA2b,
                "a-2u" => ValidationProfile::PdfA2u,
                "a-3b" => ValidationProfile::PdfA3b,
                "ua-1" => ValidationProfile::PdfUa1,
                other => return Err(format!("unknown corpus profile {other}")),
            };
            Ok(CorpusProfile { directory, profile })
        })
        .collect()
}

fn corpus_sources() -> Result<Vec<CorpusSource>, String> {
    let mut sources = corpus_profiles()?
        .into_iter()
        .map(|profile| CorpusSource {
            directory: profile.directory,
            manifest_directory: profile.directory,
            profile: CorpusSourceProfile::Fixed(profile.profile),
        })
        .collect::<Vec<_>>();
    sources.extend([
        CorpusSource {
            directory: "ISO 32000-1",
            manifest_directory: "ISO 32000-1",
            profile: CorpusSourceProfile::Iso32000,
        },
        CorpusSource {
            directory: "Isartor test files/PDFA-1b",
            manifest_directory: "Isartor test files",
            profile: CorpusSourceProfile::Fixed(ValidationProfile::PdfA1b),
        },
        CorpusSource {
            directory: "TWG test files",
            manifest_directory: "TWG test files",
            profile: CorpusSourceProfile::Twg,
        },
        CorpusSource {
            directory: "Undefined",
            manifest_directory: "Undefined",
            profile: CorpusSourceProfile::Fixed(ValidationProfile::PdfA1b),
        },
    ]);
    Ok(sources)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpectedResult {
    Pass,
    Fail,
}

impl ExpectedResult {
    const fn exit_code(self) -> i32 {
        match self {
            Self::Pass => 0,
            Self::Fail => 2,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
        }
    }
}

struct CorpusCase {
    path: PathBuf,
    profile: ValidationProfile,
    expected: ExpectedResult,
    expected_reference_rule: Option<String>,
    expected_rules: Vec<String>,
}

struct CorpusRuleExpectation {
    reference_rule: String,
    local_rules: Vec<String>,
}

const MAX_MISMATCH_DETAILS: usize = 50;
const DEFAULT_MAX_WORKERS: usize = 4;

pub(crate) fn run(args: &CorpusArgs) -> i32 {
    let rule_expectations = match load_rule_expectations(args.rule_manifest.as_deref()) {
        Ok(expectations) => expectations,
        Err(error) => {
            eprintln!("corpus error: {error}");
            return 1;
        }
    };
    let result_expectations = match load_result_expectations() {
        Ok(expectations) => expectations,
        Err(error) => {
            eprintln!("corpus error: {error}");
            return 1;
        }
    };
    let cases = match discover_cases(&args.directory, &rule_expectations, &result_expectations) {
        Ok(cases) if cases.is_empty() => {
            eprintln!(
                "corpus error: no PDF files found in '{}'",
                args.directory.display()
            );
            return 1;
        }
        Ok(cases) => cases,
        Err(error) => {
            eprintln!("corpus error: {error}");
            return 1;
        }
    };

    let limits = SafetyLimits::default();
    let spinner = Spinner::new(
        std::io::stderr().is_terminal(),
        std::env::var_os("NO_COLOR").is_none(),
        format!("Validating corpus (0/{} cases)", cases.len()),
    );
    let reports = validate_cases(&cases, &limits, args.jobs, &spinner);
    spinner.finish_and_clear();

    let mut mismatches = 0;
    let mut operational_failures = 0;
    let mut displayed_mismatches = 0;
    let mut suppressed_mismatches = 0;

    for (case, (actual, report)) in cases.iter().zip(&reports) {
        let actual = *actual;
        if actual == case.expected.exit_code()
            && expected_rules_are_reported(&case.expected_rules, report)
        {
            continue;
        }

        if actual == 1 {
            operational_failures += 1;
        } else {
            mismatches += 1;
        }
        if actual == 1 || displayed_mismatches < MAX_MISMATCH_DETAILS {
            print_mismatch(case, actual, report);
            if actual != 1 {
                displayed_mismatches += 1;
            }
        } else {
            suppressed_mismatches += 1;
        }
    }

    let matched = cases.len() - mismatches - operational_failures;
    let exit_code = if operational_failures > 0 {
        1
    } else if mismatches > 0 {
        2
    } else {
        0
    };
    eprintln!();
    eprintln!("Corpus validation");
    eprintln!("  cases:       {}", cases.len());
    eprintln!("  matched:     {matched}");
    eprintln!("  mismatches:  {mismatches}");
    eprintln!("  operational: {operational_failures}");
    eprintln!(
        "  result:      {} (exit {exit_code})",
        exit_label(exit_code)
    );
    if suppressed_mismatches > 0 {
        eprintln!(
            "  details:     suppressed {suppressed_mismatches} additional mismatches after displaying {MAX_MISMATCH_DETAILS}"
        );
    }
    exit_code
}

/// Validates every case, distributing them across a bounded pool of worker
/// threads since each case's validation is independent CPU-bound work.
/// Results are returned in the same order as `cases` regardless of which
/// worker completed them, so mismatch reporting stays deterministic.
fn validate_cases(
    cases: &[CorpusCase],
    limits: &SafetyLimits,
    jobs: Option<NonZeroUsize>,
    spinner: &Spinner,
) -> Vec<(i32, ValidationReport)> {
    let available_workers = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    let worker_count = jobs
        .map(NonZeroUsize::get)
        .unwrap_or_else(|| available_workers.min(DEFAULT_MAX_WORKERS))
        .min(cases.len().max(1));

    let completed = AtomicUsize::new(0);

    if worker_count <= 1 {
        return cases
            .iter()
            .map(|case| {
                let report = validate_case(&case.path, case.profile, limits);
                let actual = report.exit_code();
                let completed = completed.fetch_add(1, Ordering::Relaxed) + 1;
                spinner.set_message(format!(
                    "Validating corpus ({completed}/{} cases)",
                    cases.len()
                ));
                (actual, report)
            })
            .collect();
    }

    let next_index = AtomicUsize::new(0);
    let mut indexed_results: Vec<(usize, i32, ValidationReport)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..worker_count)
            .map(|_| {
                let next_index = &next_index;
                let completed = &completed;
                scope.spawn(move || {
                    let mut results = Vec::new();
                    loop {
                        let index = next_index.fetch_add(1, Ordering::Relaxed);
                        let Some(case) = cases.get(index) else {
                            break;
                        };
                        let report = validate_case(&case.path, case.profile, limits);
                        let actual = report.exit_code();
                        let completed = completed.fetch_add(1, Ordering::Relaxed) + 1;
                        spinner.set_message(format!(
                            "Validating corpus ({completed}/{} cases)",
                            cases.len()
                        ));
                        results.push((index, actual, report));
                    }
                    results
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("corpus worker thread panicked"))
            .collect()
    });

    indexed_results.sort_by_key(|(index, _, _)| *index);
    indexed_results
        .into_iter()
        .map(|(_, actual, report)| (actual, report))
        .collect()
}

fn validate_case(
    path: &Path,
    profile: ValidationProfile,
    limits: &SafetyLimits,
) -> ValidationReport {
    validate_pdf(path, Some(profile), limits).unwrap_or_else(|error| {
        let mut report = ValidationReport::from_validation_error(profile, error);
        report.source = Some(path.to_path_buf());
        report
    })
}

fn discover_cases(
    root: &Path,
    rule_expectations: &HashMap<String, CorpusRuleExpectation>,
    result_expectations: &HashMap<String, ExpectedResult>,
) -> Result<Vec<CorpusCase>, String> {
    if !root.is_dir() {
        return Err(format!(
            "corpus directory '{}' does not exist or is not a directory",
            root.display()
        ));
    }

    let mut cases = Vec::new();
    for source in corpus_sources()? {
        let directory = root.join(source.directory);
        if !directory.is_dir() {
            return Err(format!(
                "missing selected profile directory '{}'",
                directory.display()
            ));
        }

        let mut files = Vec::new();
        collect_pdf_files(&directory, &mut files)
            .map_err(|error| format!("could not scan '{}': {error}", directory.display()))?;
        if files.is_empty() {
            return Err(format!(
                "selected profile directory '{}' contains no PDF files",
                directory.display()
            ));
        }
        for path in files {
            let relative_path = path
                .strip_prefix(&directory)
                .map_err(|error| {
                    format!(
                        "could not make '{}' relative to '{}': {error}",
                        path.display(),
                        directory.display()
                    )
                })?
                .to_str()
                .ok_or_else(|| format!("corpus file has a non-UTF-8 path: '{}'", path.display()))?
                .replace(std::path::MAIN_SEPARATOR, "/");
            let expectation_key = format!("{}\t{}", source.manifest_directory, relative_path);
            let expected_from_name = expected_result(&path);
            let expected = result_expectations
                .get(&expectation_key)
                .copied()
                .or_else(|| expected_from_name.as_ref().ok().copied())
                .ok_or_else(|| {
                    expected_from_name.expect_err("missing corpus result expectation")
                })?;
            let profile = match source.profile {
                CorpusSourceProfile::Fixed(profile) => profile,
                CorpusSourceProfile::Iso32000 => iso_profile(&path)?,
                CorpusSourceProfile::Twg => twg_profile(&path)?,
            };
            let (expected_reference_rule, expected_rules) = match expected {
                ExpectedResult::Pass => {
                    if rule_expectations.contains_key(&expectation_key) {
                        return Err(format!(
                            "rule expectation manifest contains a pass file '{}', which must not have an expected failed rule",
                            path.display()
                        ));
                    }
                    (None, Vec::new())
                }
                ExpectedResult::Fail => {
                    let expectation = rule_expectations.get(&expectation_key).ok_or_else(|| {
                        format!(
                            "rule expectation manifest has no entry for failed corpus file '{}'; regenerate it for the pinned corpus revision",
                            path.display()
                        )
                    })?;
                    (
                        Some(expectation.reference_rule.clone()),
                        expectation.local_rules.clone(),
                    )
                }
            };
            cases.push(CorpusCase {
                path,
                profile,
                expected,
                expected_reference_rule,
                expected_rules,
            });
        }
    }
    cases.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(cases)
}

fn iso_profile(path: &Path) -> Result<ValidationProfile, String> {
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("corpus file has a non-UTF-8 name: '{}'", path.display()))?;
    if file_name.contains("6-8-") {
        Ok(ValidationProfile::PdfA1a)
    } else {
        Ok(ValidationProfile::PdfA1b)
    }
}

fn twg_profile(path: &Path) -> Result<ValidationProfile, String> {
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("corpus file has a non-UTF-8 name: '{}'", path.display()))?;
    let profiles = [
        ("-pdfa1-", ValidationProfile::PdfA1b),
        ("-pdfa2-", ValidationProfile::PdfA2b),
        ("-pdfa3-", ValidationProfile::PdfA3b),
    ];
    let matches = profiles
        .iter()
        .filter(|(marker, _)| file_name.contains(marker))
        .map(|(_, profile)| *profile)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [profile] => Ok(*profile),
        [] => Err(format!(
            "could not infer TWG profile from '{}'; expected '-pdfa1-', '-pdfa2-', or '-pdfa3-'",
            file_name
        )),
        _ => Err(format!("ambiguous TWG profile in '{}'", file_name)),
    }
}

fn load_rule_expectations(
    path: Option<&Path>,
) -> Result<HashMap<String, CorpusRuleExpectation>, String> {
    let contents = match path {
        Some(path) => fs::read_to_string(path).map_err(|error| {
            format!(
                "could not read rule expectation manifest '{}': {error}",
                path.display()
            )
        })?,
        None => CORPUS_RULE_EXPECTATIONS_SPEC.to_owned(),
    };
    let mut expectations = HashMap::new();
    for (line_number, line) in contents.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 4 || fields.iter().any(|field| field.is_empty()) {
            return Err(format!(
                "invalid rule expectation manifest entry on line {}",
                line_number + 1
            ));
        }
        let (Some(profile), Some(relative_path), Some(reference_rule), Some(local_rules)) =
            (fields.first(), fields.get(1), fields.get(2), fields.get(3))
        else {
            return Err(format!(
                "invalid rule expectation manifest entry on line {}",
                line_number + 1
            ));
        };
        let key = format!("{profile}\t{relative_path}");
        if expectations
            .insert(
                key,
                CorpusRuleExpectation {
                    reference_rule: (*reference_rule).to_owned(),
                    local_rules: local_rules.split(',').map(str::to_owned).collect(),
                },
            )
            .is_some()
        {
            return Err(format!(
                "duplicate rule expectation manifest entry on line {}",
                line_number + 1
            ));
        }
    }
    Ok(expectations)
}

fn load_result_expectations() -> Result<HashMap<String, ExpectedResult>, String> {
    let mut expectations = HashMap::new();
    for (line_number, line) in CORPUS_RESULT_EXPECTATIONS_SPEC.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 3 || fields.iter().any(|field| field.is_empty()) {
            return Err(format!(
                "invalid result expectation manifest entry on line {}",
                line_number + 1
            ));
        }
        let [source, relative_path, expected_field] = fields.as_slice() else {
            return Err(format!(
                "invalid result expectation manifest entry on line {}",
                line_number + 1
            ));
        };
        let expected = match *expected_field {
            "pass" => ExpectedResult::Pass,
            "fail" => ExpectedResult::Fail,
            other => {
                return Err(format!(
                    "unknown result expectation '{other}' on line {}",
                    line_number + 1
                ));
            }
        };
        let key = format!("{source}\t{relative_path}");
        if expectations.insert(key, expected).is_some() {
            return Err(format!(
                "duplicate result expectation manifest entry on line {}",
                line_number + 1
            ));
        }
    }
    Ok(expectations)
}

fn expected_rules_are_reported(expected_rules: &[String], report: &ValidationReport) -> bool {
    expected_rules.is_empty()
        || expected_rules.iter().any(|expected_rule| {
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == *expected_rule)
        })
}

fn collect_pdf_files(directory: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_pdf_files(&path, files)?;
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(OsStr::to_str)
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn expected_result(path: &Path) -> Result<ExpectedResult, String> {
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("corpus file has a non-UTF-8 name: '{}'", path.display()))?;
    let stem = path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("corpus file has no valid stem: '{}'", path.display()))?;
    let markers = stem
        .split('-')
        .filter_map(|part| match part {
            "pass" => Some(ExpectedResult::Pass),
            "fail" => Some(ExpectedResult::Fail),
            _ => None,
        })
        .collect::<Vec<_>>();
    match markers.as_slice() {
        [expected] => Ok(*expected),
        [] => Err(format!(
            "could not infer pass/fail expectation from '{}'; expected '-pass-' or '-fail-' in the filename",
            file_name
        )),
        _ => Err(format!(
            "ambiguous pass/fail expectation in '{}'; expected exactly one marker",
            file_name
        )),
    }
}

fn print_mismatch(case: &CorpusCase, actual: i32, report: &ValidationReport) {
    eprintln!();
    eprintln!("Corpus mismatch");
    eprintln!("  file:     {}", case.path.display());
    eprintln!("  profile:  {}", case.profile);
    eprintln!(
        "  expected: {} (exit {})",
        case.expected.as_str(),
        case.expected.exit_code()
    );
    if !case.expected_rules.is_empty() {
        eprintln!("  expected rules: {}", case.expected_rules.join(", "));
    }
    if let Some(reference_rule) = &case.expected_reference_rule {
        eprintln!("  veraPDF rule:  {reference_rule}");
    }
    eprintln!("  actual:   {} (exit {actual})", exit_label(actual));
    if report.failures.is_empty() {
        eprintln!("  failures: none reported");
    } else {
        eprintln!("  failures:");
        for failure in &report.failures {
            eprintln!("    - rule:     {}", failure.rule_id);
            eprintln!("      category: {}", category_label(failure.category));
            eprintln!("      message:  {}", failure.message);
        }
    }
}

fn exit_label(exit_code: i32) -> &'static str {
    match exit_code {
        0 => "pass",
        1 => "operational failure",
        2 => "fail",
        _ => "unexpected result",
    }
}

fn category_label(category: page_validation::FailureCategory) -> &'static str {
    match category {
        page_validation::FailureCategory::Operational => "operational",
        page_validation::FailureCategory::Parser => "parser",
        page_validation::FailureCategory::Metadata => "metadata",
        page_validation::FailureCategory::Conformance => "conformance",
    }
}

#[cfg(test)]
mod tests {
    use super::{ExpectedResult, expected_result, expected_rules_are_reported};
    use page_validation::{SafetyLimits, ValidationProfile, ValidationReport, validate_pdf_bytes};
    use std::path::Path;

    #[test]
    fn reads_expected_result_from_corpus_filename() {
        assert_eq!(
            expected_result(Path::new("veraPDF test suite 6-3-3-1-t01-fail-b.pdf")),
            Ok(ExpectedResult::Fail)
        );
        assert_eq!(
            expected_result(Path::new("veraPDF test suite 6-3-3-1-t01-pass-a.pdf")),
            Ok(ExpectedResult::Pass)
        );
    }

    #[test]
    fn rejects_missing_or_ambiguous_expected_result() {
        expected_result(Path::new("case.pdf")).unwrap_err();
        expected_result(Path::new("case-pass-fail.pdf")).unwrap_err();
    }

    #[test]
    fn infers_profiles_from_twg_file_names() {
        assert_eq!(
            super::twg_profile(Path::new("TWG test suite A001-pdfa1-fail-a.pdf")),
            Ok(ValidationProfile::PdfA1b)
        );
        assert_eq!(
            super::twg_profile(Path::new("TWG test suite A002-pdfa2-pass-a.pdf")),
            Ok(ValidationProfile::PdfA2b)
        );
        assert_eq!(
            super::twg_profile(Path::new("TWG test suite A027-pdfa3-fail-a.pdf")),
            Ok(ValidationProfile::PdfA3b)
        );
    }

    #[test]
    fn requires_the_expected_rule_in_the_report() {
        let error = validate_pdf_bytes(
            b"not a PDF",
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect_err("invalid PDF should be rejected");
        let report = ValidationReport::from_validation_error(ValidationProfile::PdfA1b, error);

        assert!(expected_rules_are_reported(
            &["PDF-PARSE-001".to_owned()],
            &report
        ));
        assert!(!expected_rules_are_reported(
            &["PDFA1B-HEADER-001".to_owned()],
            &report
        ));
        assert!(expected_rules_are_reported(&[], &report));
    }
}
