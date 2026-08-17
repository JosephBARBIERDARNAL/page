use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Args;
use page_validation::{
    SafetyLimits, ValidationProfile, ValidationReport, validate_file_with_profile,
};

#[derive(Debug, Args)]
pub(crate) struct CorpusArgs {
    /// Root directory of a veraPDF corpus checkout.
    #[arg(value_name = "DIRECTORY")]
    pub(crate) directory: PathBuf,
}

#[derive(Clone, Copy)]
struct CorpusProfile {
    directory: &'static str,
    profile: ValidationProfile,
}

const CORPUS_PROFILES: [CorpusProfile; 6] = [
    CorpusProfile {
        directory: "PDF_A-1a",
        profile: ValidationProfile::PdfA1a,
    },
    CorpusProfile {
        directory: "PDF_A-1b",
        profile: ValidationProfile::PdfA1b,
    },
    CorpusProfile {
        directory: "PDF_A-2a",
        profile: ValidationProfile::PdfA2a,
    },
    CorpusProfile {
        directory: "PDF_A-2b",
        profile: ValidationProfile::PdfA2b,
    },
    CorpusProfile {
        directory: "PDF_A-2u",
        profile: ValidationProfile::PdfA2u,
    },
    CorpusProfile {
        directory: "PDF_A-3b",
        profile: ValidationProfile::PdfA3b,
    },
];

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
}

const MAX_MISMATCH_DETAILS: usize = 50;

pub(crate) fn run(args: &CorpusArgs) -> i32 {
    let cases = match discover_cases(&args.directory) {
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
    let mut mismatches = 0;
    let mut operational_failures = 0;
    let mut displayed_mismatches = 0;
    let mut suppressed_mismatches = 0;

    for case in &cases {
        let report = validate_file_with_profile(&case.path, case.profile, &limits);
        let actual = report.exit_code();
        if actual == case.expected.exit_code() {
            continue;
        }

        if actual == 1 {
            operational_failures += 1;
        } else {
            mismatches += 1;
        }
        if actual == 1 || displayed_mismatches < MAX_MISMATCH_DETAILS {
            print_mismatch(case, actual, &report);
            if actual != 1 {
                displayed_mismatches += 1;
            }
        } else {
            suppressed_mismatches += 1;
        }
    }

    let matched = cases.len() - mismatches - operational_failures;
    eprintln!(
        "\n\nCorpus: {matched}/{} cases matched, {mismatches} mismatches, {operational_failures} operational failures",
        cases.len()
    );
    if suppressed_mismatches > 0 {
        eprintln!(
            "Corpus: suppressed {suppressed_mismatches} additional mismatch details after displaying {MAX_MISMATCH_DETAILS}"
        );
    }
    if operational_failures > 0 {
        1
    } else if mismatches > 0 {
        2
    } else {
        0
    }
}

fn discover_cases(root: &Path) -> Result<Vec<CorpusCase>, String> {
    if !root.is_dir() {
        return Err(format!(
            "corpus directory '{}' does not exist or is not a directory",
            root.display()
        ));
    }

    let mut cases = Vec::new();
    for corpus_profile in CORPUS_PROFILES {
        let directory = root.join(corpus_profile.directory);
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
            let expected = expected_result(&path)?;
            cases.push(CorpusCase {
                path,
                profile: corpus_profile.profile,
                expected,
            });
        }
    }
    cases.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(cases)
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
    eprintln!(
        "corpus mismatch: '{}' ({}) expected page exit {}, got {}",
        case.path.display(),
        case.expected.as_str(),
        case.expected.exit_code(),
        actual
    );
    for failure in &report.failures {
        eprintln!(
            "  [{}] {:?}: {}",
            failure.rule_id, failure.category, failure.message
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{ExpectedResult, expected_result};
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
        assert!(expected_result(Path::new("case.pdf")).is_err());
        assert!(expected_result(Path::new("case-pass-fail.pdf")).is_err());
    }
}
