//! Differential validation against the pinned veraPDF reference.
//!
//! This module compares the crate's deliberately incomplete PDF/A-1, PDF/A-2, and PDF/A-3 checks
//! with veraPDF. It does not turn the local validator into a complete
//! conformance checker.

use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

use crate::{
    FailureCategory, SafetyLimits, ValidationProfile, ValidationReport, validate_file_with_profile,
};

pub const PINNED_VERAPDF_VERSION: &str = "1.30.2";
pub const PINNED_VERAPDF_PROFILE: ReferenceProfile = ReferenceProfile::PdfA1b;
pub const DEFAULT_TIMEOUT_MILLIS: u64 = 30_000;
pub const DEFAULT_MAX_REPORT_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024;
pub const DEFAULT_BATCH_SIZE: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ReferenceProfile {
    #[serde(rename = "1a")]
    PdfA1a,
    #[serde(rename = "1b")]
    PdfA1b,
    #[serde(rename = "2a")]
    PdfA2a,
    #[serde(rename = "2b")]
    PdfA2b,
    #[serde(rename = "2u")]
    PdfA2u,
    #[serde(rename = "3a")]
    PdfA3a,
    #[serde(rename = "3b")]
    PdfA3b,
    #[serde(rename = "3u")]
    PdfA3u,
    #[serde(rename = "ua1")]
    PdfUa1,
}

impl ReferenceProfile {
    pub const fn as_verapdf_flavour(self) -> &'static str {
        match self {
            Self::PdfA1a => "1a",
            Self::PdfA1b => "1b",
            Self::PdfA2a => "2a",
            Self::PdfA2b => "2b",
            Self::PdfA2u => "2u",
            Self::PdfA3a => "3a",
            Self::PdfA3b => "3b",
            Self::PdfA3u => "3u",
            Self::PdfUa1 => "ua1",
        }
    }

    const fn expected_profile_name(self) -> &'static str {
        match self {
            Self::PdfA1a => "PDF/A-1a validation profile",
            Self::PdfA1b => "PDF/A-1b validation profile",
            Self::PdfA2a => "PDF/A-2a validation profile",
            Self::PdfA2b => "PDF/A-2b validation profile",
            Self::PdfA2u => "PDF/A-2u validation profile",
            Self::PdfA3a => "PDF/A-3a validation profile",
            Self::PdfA3b => "PDF/A-3b validation profile",
            Self::PdfA3u => "PDF/A-3u validation profile",
            Self::PdfUa1 => "PDF/UA-1 validation profile",
        }
    }
}

impl fmt::Display for ReferenceProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_verapdf_flavour())
    }
}

impl From<ReferenceProfile> for ValidationProfile {
    fn from(profile: ReferenceProfile) -> Self {
        match profile {
            ReferenceProfile::PdfA1a => Self::PdfA1a,
            ReferenceProfile::PdfA1b => Self::PdfA1b,
            ReferenceProfile::PdfA2a => Self::PdfA2a,
            ReferenceProfile::PdfA2b => Self::PdfA2b,
            ReferenceProfile::PdfA2u => Self::PdfA2u,
            ReferenceProfile::PdfA3a => Self::PdfA3a,
            ReferenceProfile::PdfA3b => Self::PdfA3b,
            ReferenceProfile::PdfA3u => Self::PdfA3u,
            ReferenceProfile::PdfUa1 => Self::PdfUa1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReferenceConfig {
    pub executable: PathBuf,
    pub expected_version: String,
    pub profile: ReferenceProfile,
    pub timeout_millis: u64,
    pub max_report_bytes: usize,
    pub max_diagnostic_bytes: usize,
    pub batch_size: usize,
    pub coverage_gap_policy: CoverageGapPolicy,
}

impl ReferenceConfig {
    pub fn pinned(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            expected_version: PINNED_VERAPDF_VERSION.to_owned(),
            profile: PINNED_VERAPDF_PROFILE,
            timeout_millis: DEFAULT_TIMEOUT_MILLIS,
            max_report_bytes: DEFAULT_MAX_REPORT_BYTES,
            max_diagnostic_bytes: DEFAULT_MAX_DIAGNOSTIC_BYTES,
            batch_size: DEFAULT_BATCH_SIZE,
            coverage_gap_policy: CoverageGapPolicy::AllowDuringDevelopment,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverageGapPolicy {
    AllowDuringDevelopment,
    RejectForCompleteProfile,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReferenceIdentity {
    pub product: &'static str,
    pub version: String,
    pub profile: ReferenceProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceParseState {
    Processed,
    RejectedMalformed,
    RejectedEncrypted,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ReferenceRuleId {
    pub specification: String,
    pub clause: String,
    pub test_number: u64,
}

impl fmt::Display for ReferenceRuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.specification, self.clause, self.test_number
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReferenceDiagnostics {
    pub exit_code: Option<i32>,
    pub stdout_excerpt: String,
    pub stderr_excerpt: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReferenceResult {
    pub compliant: Option<bool>,
    pub parse_state: ReferenceParseState,
    pub profile_name: Option<String>,
    pub failed_rule_ids: Vec<ReferenceRuleId>,
    pub failed_rules: u64,
    pub failed_checks: u64,
    pub task_exception: Option<String>,
    pub diagnostics: ReferenceDiagnostics,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonClassification {
    Agreement,
    BothNoncompliant,
    CoverageGap,
    LocalFalseNegative,
    LocalParserDiscrepancy,
    ReferenceParserDiscrepancy,
    Operational,
}

impl ComparisonClassification {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Agreement => "agreement",
            Self::BothNoncompliant => "both_noncompliant",
            Self::CoverageGap => "coverage_gap",
            Self::LocalFalseNegative => "local_false_negative",
            Self::LocalParserDiscrepancy => "local_parser_discrepancy",
            Self::ReferenceParserDiscrepancy => "reference_parser_discrepancy",
            Self::Operational => "operational",
        }
    }

    pub const fn is_acceptable(self) -> bool {
        matches!(
            self,
            Self::Agreement | Self::BothNoncompliant | Self::CoverageGap
        )
    }

    pub const fn is_acceptable_under(self, policy: CoverageGapPolicy) -> bool {
        match (self, policy) {
            (Self::CoverageGap, CoverageGapPolicy::RejectForCompleteProfile) => false,
            _ => self.is_acceptable(),
        }
    }

    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Operational => 1,
            Self::Agreement | Self::BothNoncompliant | Self::CoverageGap => 0,
            Self::LocalFalseNegative
            | Self::LocalParserDiscrepancy
            | Self::ReferenceParserDiscrepancy => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationalFailureKind {
    ReferenceUnavailable,
    VersionMismatch,
    Timeout,
    InvalidReferenceReport,
    ReferenceProcess,
    LocalOperational,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OperationalFailure {
    pub kind: OperationalFailureKind,
    pub message: String,
    pub diagnostics: Option<ReferenceDiagnostics>,
}

impl fmt::Display for OperationalFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Operational failure ({:?}): {}",
            self.kind, self.message
        )
    }
}

#[derive(Debug, Serialize)]
pub struct DifferentialReport {
    pub file: PathBuf,
    pub reference_identity: ReferenceIdentity,
    pub classification: ComparisonClassification,
    pub acceptable: bool,
    pub summary: String,
    pub local_report: ValidationReport,
    pub reference_result: Option<ReferenceResult>,
    pub operational_failure: Option<OperationalFailure>,
}

impl DifferentialReport {
    pub fn exit_code(&self) -> i32 {
        match self.classification {
            ComparisonClassification::Operational => 1,
            _ if self.acceptable => 0,
            _ => 2,
        }
    }
}

impl fmt::Display for DifferentialReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "File: {}", self.file.display())?;
        writeln!(
            formatter,
            "Reference: veraPDF {} (profile {})",
            self.reference_identity.version, self.reference_identity.profile
        )?;
        writeln!(
            formatter,
            "Classification: {}",
            self.classification.as_str()
        )?;
        writeln!(formatter, "Summary: {}", self.summary)?;
        if let Some(reference) = &self.reference_result {
            writeln!(
                formatter,
                "veraPDF: parse={:?}, compliant={:?}, failed rules={}, failed checks={}",
                reference.parse_state,
                reference.compliant,
                reference.failed_rules,
                reference.failed_checks
            )?;
            if !reference.failed_rule_ids.is_empty() {
                let ids = reference
                    .failed_rule_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(formatter, "veraPDF failed rule IDs: {ids}")?;
            }
        }
        if let Some(failure) = &self.operational_failure {
            writeln!(formatter, "{failure}")?;
        }
        writeln!(
            formatter,
            "Local implemented checks passed: {}",
            self.local_report.checks_passed
        )?;
        if self.classification == ComparisonClassification::CoverageGap {
            writeln!(
                formatter,
                "Warning: coverage_gap means only the local subset passed; the PDF is not compliant with the selected PDF/A profile."
            )?;
        }
        Ok(())
    }
}

pub struct DifferentialRunner {
    config: ReferenceConfig,
    identity: ReferenceIdentity,
}

impl DifferentialRunner {
    pub fn new(config: ReferenceConfig) -> Result<Self, OperationalFailure> {
        let mut command = Command::new(&config.executable);
        command.arg("--version");
        let captured = run_command_capped(
            &mut command,
            Duration::from_millis(config.timeout_millis),
            config.max_diagnostic_bytes,
        )
        .map_err(|failure| process_failure_to_operational(failure, config.max_diagnostic_bytes))?;
        let diagnostics = diagnostics(&captured, config.max_diagnostic_bytes);
        if !captured.status.success() {
            return Err(OperationalFailure {
                kind: OperationalFailureKind::ReferenceProcess,
                message: format!(
                    "veraPDF --version exited with status {:?}",
                    captured.status.code()
                ),
                diagnostics: Some(diagnostics),
            });
        }
        if captured.stdout.truncated {
            return Err(OperationalFailure {
                kind: OperationalFailureKind::InvalidReferenceReport,
                message: "veraPDF version output exceeded the configured diagnostic limit"
                    .to_owned(),
                diagnostics: Some(diagnostics),
            });
        }
        let actual_version =
            parse_version_output(&captured.stdout.bytes).map_err(|message| OperationalFailure {
                kind: OperationalFailureKind::InvalidReferenceReport,
                message,
                diagnostics: Some(diagnostics.clone()),
            })?;
        verify_cli_version(&actual_version, &config.expected_version).map_err(|message| {
            OperationalFailure {
                kind: OperationalFailureKind::VersionMismatch,
                message,
                diagnostics: Some(diagnostics),
            }
        })?;
        let identity = ReferenceIdentity {
            product: "veraPDF",
            version: config.expected_version.clone(),
            profile: config.profile,
        };
        Ok(Self { config, identity })
    }

    pub fn compare_file(&self, path: &Path, limits: &SafetyLimits) -> DifferentialReport {
        let local_report = validate_file_with_profile(path, self.config.profile.into(), limits);
        if local_report.has_operational_failure() {
            return self.operational_report(
                path,
                local_report,
                OperationalFailure {
                    kind: OperationalFailureKind::LocalOperational,
                    message: "the local validator could not read or safely process the input"
                        .to_owned(),
                    diagnostics: None,
                },
            );
        }

        match self.run_reference(path) {
            Ok(reference_result) => {
                let classification = classify(&local_report, &reference_result);
                DifferentialReport {
                    file: path.to_owned(),
                    reference_identity: self.identity.clone(),
                    classification,
                    acceptable: classification.is_acceptable_under(self.config.coverage_gap_policy),
                    summary: classification_summary(classification).to_owned(),
                    local_report,
                    reference_result: Some(reference_result),
                    operational_failure: None,
                }
            }
            Err(failure) => self.operational_report(path, local_report, failure),
        }
    }

    /// Compare explicit PDF paths in bounded veraPDF batches while preserving
    /// input order and one [`DifferentialReport`] per path.
    pub fn compare_files(
        &self,
        paths: &[PathBuf],
        limits: &SafetyLimits,
    ) -> Vec<DifferentialReport> {
        let local_reports = paths
            .iter()
            .map(|path| validate_file_with_profile(path, self.config.profile.into(), limits))
            .collect::<Vec<_>>();
        let reference_indices = local_reports
            .iter()
            .enumerate()
            .filter_map(|(index, report)| (!report.has_operational_failure()).then_some(index))
            .collect::<Vec<_>>();
        let mut reference_outcomes = vec![None; paths.len()];

        for indices in reference_indices.chunks(self.config.batch_size.max(1)) {
            let batch_paths = indices
                .iter()
                .filter_map(|index| paths.get(*index).map(PathBuf::as_path))
                .collect::<Vec<_>>();
            match self.run_reference_batch(&batch_paths) {
                Ok(outcomes) => {
                    debug_assert_eq!(outcomes.len(), indices.len());
                    for (index, outcome) in indices.iter().zip(outcomes) {
                        if let Some(slot) = reference_outcomes.get_mut(*index) {
                            *slot = Some(outcome);
                        }
                    }
                }
                Err(failure) => {
                    for index in indices {
                        if let Some(slot) = reference_outcomes.get_mut(*index) {
                            *slot = Some(Err(failure.clone()));
                        }
                    }
                }
            }
        }

        paths
            .iter()
            .zip(local_reports)
            .zip(reference_outcomes)
            .map(|((path, local_report), reference_outcome)| {
                if local_report.has_operational_failure() {
                    return self.operational_report(
                        path,
                        local_report,
                        OperationalFailure {
                            kind: OperationalFailureKind::LocalOperational,
                            message:
                                "the local validator could not read or safely process the input"
                                    .to_owned(),
                            diagnostics: None,
                        },
                    );
                }
                match reference_outcome.expect("reference outcome for processable input") {
                    Ok(reference_result) => {
                        self.comparison_report(path, local_report, reference_result)
                    }
                    Err(failure) => self.operational_report(path, local_report, failure),
                }
            })
            .collect()
    }

    fn comparison_report(
        &self,
        path: &Path,
        local_report: ValidationReport,
        reference_result: ReferenceResult,
    ) -> DifferentialReport {
        let classification = classify(&local_report, &reference_result);
        DifferentialReport {
            file: path.to_owned(),
            reference_identity: self.identity.clone(),
            classification,
            acceptable: classification.is_acceptable_under(self.config.coverage_gap_policy),
            summary: classification_summary(classification).to_owned(),
            local_report,
            reference_result: Some(reference_result),
            operational_failure: None,
        }
    }

    fn operational_report(
        &self,
        path: &Path,
        local_report: ValidationReport,
        failure: OperationalFailure,
    ) -> DifferentialReport {
        DifferentialReport {
            file: path.to_owned(),
            reference_identity: self.identity.clone(),
            classification: ComparisonClassification::Operational,
            acceptable: false,
            summary: classification_summary(ComparisonClassification::Operational).to_owned(),
            local_report,
            reference_result: None,
            operational_failure: Some(failure),
        }
    }

    fn run_reference(&self, path: &Path) -> Result<ReferenceResult, OperationalFailure> {
        self.run_reference_batch(&[path])?
            .pop()
            .expect("one requested veraPDF job")
    }

    fn run_reference_batch(
        &self,
        paths: &[&Path],
    ) -> Result<Vec<Result<ReferenceResult, OperationalFailure>>, OperationalFailure> {
        let mut command = build_validation_command(&self.config, paths);
        let captured = run_command_capped(
            &mut command,
            Duration::from_millis(self.config.timeout_millis),
            self.config.max_report_bytes,
        )
        .map_err(|failure| {
            process_failure_to_operational(failure, self.config.max_diagnostic_bytes)
        })?;
        let report_diagnostics = diagnostics(&captured, self.config.max_diagnostic_bytes);
        if captured.stdout.truncated {
            return Err(OperationalFailure {
                kind: OperationalFailureKind::InvalidReferenceReport,
                message: format!(
                    "veraPDF JSON exceeded the {}-byte report limit",
                    self.config.max_report_bytes
                ),
                diagnostics: Some(report_diagnostics),
            });
        }
        parse_reference_reports(
            &captured.stdout.bytes,
            &self.identity,
            report_diagnostics.clone(),
            paths.len(),
        )
        .map(|outcomes| {
            outcomes
                .into_iter()
                .map(|outcome| {
                    outcome.map_err(|message| OperationalFailure {
                        kind: OperationalFailureKind::InvalidReferenceReport,
                        message,
                        diagnostics: Some(report_diagnostics.clone()),
                    })
                })
                .collect()
        })
        .map_err(|message| OperationalFailure {
            kind: OperationalFailureKind::InvalidReferenceReport,
            message,
            diagnostics: Some(report_diagnostics),
        })
    }
}

fn classification_summary(classification: ComparisonClassification) -> &'static str {
    match classification {
        ComparisonClassification::Agreement => {
            "veraPDF is compliant and all local implemented checks pass"
        }
        ComparisonClassification::BothNoncompliant => {
            "both validators reject the input or report noncompliance"
        }
        ComparisonClassification::CoverageGap => {
            "the local subset passes but veraPDF fails additional rules"
        }
        ComparisonClassification::LocalFalseNegative => {
            "veraPDF passes but at least one local implemented check fails"
        }
        ComparisonClassification::LocalParserDiscrepancy => {
            "the local parser rejects a file that veraPDF processes"
        }
        ComparisonClassification::ReferenceParserDiscrepancy => {
            "veraPDF cannot process a file that the local parser processes"
        }
        ComparisonClassification::Operational => {
            "the comparison could not run because of an operational failure"
        }
    }
}

pub fn classify(local: &ValidationReport, reference: &ReferenceResult) -> ComparisonClassification {
    let local_parser_rejected = local
        .failures
        .iter()
        .any(|failure| failure.category == FailureCategory::Parser);

    match reference.parse_state {
        ReferenceParseState::Processed => {
            if local_parser_rejected {
                ComparisonClassification::LocalParserDiscrepancy
            } else {
                match (reference.compliant, local.checks_passed) {
                    (Some(true), true) => ComparisonClassification::Agreement,
                    (Some(true), false) => ComparisonClassification::LocalFalseNegative,
                    (Some(false), true) => ComparisonClassification::CoverageGap,
                    (Some(false), false) => ComparisonClassification::BothNoncompliant,
                    (None, _) => ComparisonClassification::Operational,
                }
            }
        }
        ReferenceParseState::RejectedMalformed | ReferenceParseState::RejectedEncrypted => {
            if local_parser_rejected {
                ComparisonClassification::BothNoncompliant
            } else {
                ComparisonClassification::ReferenceParserDiscrepancy
            }
        }
    }
}

pub fn aggregate_exit_code(reports: &[DifferentialReport]) -> i32 {
    if reports.iter().any(|report| report.exit_code() == 1) {
        1
    } else if reports.iter().any(|report| report.exit_code() == 2) {
        2
    } else {
        0
    }
}

fn build_validation_command(config: &ReferenceConfig, paths: &[&Path]) -> Command {
    let mut command = Command::new(&config.executable);
    command.args([
        "--loglevel",
        "0",
        "--format",
        "json",
        "--flavour",
        config.profile.as_verapdf_flavour(),
    ]);
    command.args(paths);
    command
}

fn parse_version_output(bytes: &[u8]) -> Result<String, String> {
    let output = std::str::from_utf8(bytes)
        .map_err(|error| format!("veraPDF version output is not UTF-8: {error}"))?;
    let version = output
        .lines()
        .find_map(|line| line.strip_prefix("veraPDF "))
        .ok_or_else(|| {
            "veraPDF version output has no anchored `veraPDF VERSION` line".to_owned()
        })?;
    if version.is_empty()
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.' || byte == b'-')
    {
        return Err(format!("invalid veraPDF version token {version:?}"));
    }
    Ok(version.to_owned())
}

fn verify_version(actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "veraPDF version mismatch: expected {expected}, found {actual}"
        ))
    }
}

// The 1.30.2 distribution's CLI banner reports the applications component as
// 1.30.0, while validation JSON reports the core/model release as 1.30.2.
// The latter is the version used for reference identity and report parsing.
fn verify_cli_version(actual: &str, expected: &str) -> Result<(), String> {
    if expected == "1.30.2" && actual == "1.30.0" {
        return Ok(());
    }
    verify_version(actual, expected)
}

#[derive(Debug, Deserialize)]
struct RawEnvelope {
    report: RawReport,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawReport {
    build_information: RawBuildInformation,
    jobs: Vec<RawJob>,
    batch_summary: RawBatchSummary,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBuildInformation {
    release_details: Vec<RawReleaseDetail>,
}

#[derive(Debug, Deserialize)]
struct RawReleaseDetail {
    id: String,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawJob {
    validation_result: Option<Vec<RawValidationResult>>,
    task_exception: Option<RawTaskException>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawValidationResult {
    details: RawValidationDetails,
    profile_name: String,
    compliant: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawValidationDetails {
    failed_rules: u64,
    failed_checks: u64,
    #[serde(default)]
    rule_summaries: Vec<RawRuleSummary>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRuleSummary {
    specification: String,
    clause: String,
    test_number: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTaskException {
    #[serde(rename = "type")]
    exception_type: String,
    exception_message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBatchSummary {
    failed_encrypted_jobs: u64,
    failed_parsing_jobs: u64,
    /// Count of unexpected internal veraPDF exceptions during the batch.
    /// Some malformed-structure failures (confirmed: a page-tree node with
    /// a missing or unrecognized `/Type`) surface as a `PARSE` task
    /// exception counted here rather than in `failed_parsing_jobs`.
    #[serde(default)]
    vera_exceptions: u64,
}

#[cfg(test)]
fn parse_reference_report(
    bytes: &[u8],
    expected_identity: &ReferenceIdentity,
    diagnostics: ReferenceDiagnostics,
) -> Result<ReferenceResult, String> {
    parse_reference_reports(bytes, expected_identity, diagnostics, 1)?
        .pop()
        .expect("one requested veraPDF job")
}

fn parse_reference_reports(
    bytes: &[u8],
    expected_identity: &ReferenceIdentity,
    diagnostics: ReferenceDiagnostics,
    expected_jobs: usize,
) -> Result<Vec<Result<ReferenceResult, String>>, String> {
    let raw: RawEnvelope =
        serde_json::from_slice(bytes).map_err(|error| format!("invalid veraPDF JSON: {error}"))?;
    let core = raw
        .report
        .build_information
        .release_details
        .iter()
        .find(|detail| detail.id == "core")
        .ok_or_else(|| "veraPDF JSON has no core release detail".to_owned())?;
    verify_version(&core.version, &expected_identity.version)?;
    if raw.report.jobs.len() != expected_jobs {
        return Err(format!(
            "expected {expected_jobs} veraPDF jobs, found {}",
            raw.report.jobs.len()
        ));
    }
    Ok(raw
        .report
        .jobs
        .into_iter()
        .map(|job| {
            parse_reference_job(
                job,
                expected_identity,
                diagnostics.clone(),
                &raw.report.batch_summary,
            )
        })
        .collect())
}

fn parse_reference_job(
    job: RawJob,
    expected_identity: &ReferenceIdentity,
    diagnostics: ReferenceDiagnostics,
    batch_summary: &RawBatchSummary,
) -> Result<ReferenceResult, String> {
    match (job.validation_result, job.task_exception) {
        (Some(mut results), None) if results.len() == 1 => {
            let result = results.pop().expect("length checked");
            if result.profile_name != expected_identity.profile.expected_profile_name() {
                return Err(format!(
                    "unexpected veraPDF profile name {:?}, expected {:?}",
                    result.profile_name,
                    expected_identity.profile.expected_profile_name()
                ));
            }
            let mut failed_rule_ids = result
                .details
                .rule_summaries
                .into_iter()
                .map(|summary| ReferenceRuleId {
                    specification: summary.specification,
                    clause: summary.clause,
                    test_number: summary.test_number,
                })
                .collect::<Vec<_>>();
            failed_rule_ids.sort();
            failed_rule_ids.dedup();
            Ok(ReferenceResult {
                compliant: Some(result.compliant),
                parse_state: ReferenceParseState::Processed,
                profile_name: Some(result.profile_name),
                failed_rule_ids,
                failed_rules: result.details.failed_rules,
                failed_checks: result.details.failed_checks,
                task_exception: None,
                diagnostics,
            })
        }
        (None, Some(exception)) => {
            if exception.exception_type != "PARSE" {
                return Err(format!(
                    "unsupported veraPDF task exception type {:?}",
                    exception.exception_type
                ));
            }
            let message_is_encrypted = exception
                .exception_message
                .to_ascii_lowercase()
                .contains("encrypt");
            let parse_state = if message_is_encrypted && batch_summary.failed_encrypted_jobs > 0 {
                ReferenceParseState::RejectedEncrypted
            } else if batch_summary.failed_parsing_jobs > 0 || batch_summary.vera_exceptions > 0 {
                ReferenceParseState::RejectedMalformed
            } else {
                return Err(
                    "veraPDF PARSE exception was not reflected in the batch summary".to_owned(),
                );
            };
            Ok(ReferenceResult {
                compliant: None,
                parse_state,
                profile_name: None,
                failed_rule_ids: Vec::new(),
                failed_rules: 0,
                failed_checks: 0,
                task_exception: Some(exception.exception_message),
                diagnostics,
            })
        }
        (Some(results), None) => Err(format!(
            "expected exactly one validation result, found {}",
            results.len()
        )),
        (Some(_), Some(_)) => {
            Err("veraPDF job contains both validationResult and taskException".to_owned())
        }
        (None, None) => {
            Err("veraPDF job contains neither validationResult nor taskException".to_owned())
        }
    }
}

#[derive(Debug)]
struct CapturedStream {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug)]
struct CapturedProcess {
    status: ExitStatus,
    stdout: CapturedStream,
    stderr: CapturedStream,
}

enum ProcessFailure {
    Spawn(io::Error),
    Wait(io::Error),
    Timeout(CapturedProcess),
    Read(&'static str, String),
}

fn run_command_capped(
    command: &mut Command,
    timeout: Duration,
    cap: usize,
) -> Result<CapturedProcess, ProcessFailure> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(ProcessFailure::Spawn)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ProcessFailure::Read("stdout", "pipe was not created".to_owned()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ProcessFailure::Read("stderr", "pipe was not created".to_owned()))?;
    let stdout_thread = thread::spawn(move || read_capped(stdout, cap));
    let stderr_thread = thread::spawn(move || read_capped(stderr, cap));

    let waited = match child.wait_timeout(timeout) {
        Ok(waited) => waited,
        Err(error) => {
            drop(child.kill());
            drop(child.wait());
            drop(stdout_thread.join());
            drop(stderr_thread.join());
            return Err(ProcessFailure::Wait(error));
        }
    };
    let status = match waited {
        Some(status) => status,
        None => {
            drop(child.kill());
            let status = child.wait().map_err(ProcessFailure::Wait)?;
            let (stdout, stderr) = join_reader_threads(stdout_thread, stderr_thread)?;
            return Err(ProcessFailure::Timeout(CapturedProcess {
                status,
                stdout,
                stderr,
            }));
        }
    };
    let (stdout, stderr) = join_reader_threads(stdout_thread, stderr_thread)?;
    Ok(CapturedProcess {
        status,
        stdout,
        stderr,
    })
}

fn join_reader_threads(
    stdout_thread: thread::JoinHandle<io::Result<CapturedStream>>,
    stderr_thread: thread::JoinHandle<io::Result<CapturedStream>>,
) -> Result<(CapturedStream, CapturedStream), ProcessFailure> {
    let stdout = stdout_thread
        .join()
        .map_err(|error| {
            ProcessFailure::Read("stdout", format!("reader thread panicked: {error:?}"))
        })?
        .map_err(|error| ProcessFailure::Read("stdout", error.to_string()))?;
    let stderr = stderr_thread
        .join()
        .map_err(|error| {
            ProcessFailure::Read("stderr", format!("reader thread panicked: {error:?}"))
        })?
        .map_err(|error| ProcessFailure::Read("stderr", error.to_string()))?;
    Ok((stdout, stderr))
}

fn read_capped(mut reader: impl Read, cap: usize) -> io::Result<CapturedStream> {
    let mut bytes = Vec::with_capacity(cap.min(64 * 1024));
    let mut truncated = false;
    let mut buffer = [0u8; 8192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = cap.saturating_sub(bytes.len());
        let retained = count.min(remaining);
        bytes.extend_from_slice(buffer.get(..retained).unwrap_or_default());
        if retained < count {
            truncated = true;
        }
    }
    Ok(CapturedStream { bytes, truncated })
}

fn diagnostics(captured: &CapturedProcess, excerpt_limit: usize) -> ReferenceDiagnostics {
    ReferenceDiagnostics {
        exit_code: captured.status.code(),
        stdout_excerpt: excerpt(&captured.stdout.bytes, excerpt_limit),
        stderr_excerpt: excerpt(&captured.stderr.bytes, excerpt_limit),
        stdout_truncated: captured.stdout.truncated || captured.stdout.bytes.len() > excerpt_limit,
        stderr_truncated: captured.stderr.truncated || captured.stderr.bytes.len() > excerpt_limit,
    }
}

fn excerpt(bytes: &[u8], limit: usize) -> String {
    String::from_utf8_lossy(bytes.get(..bytes.len().min(limit)).unwrap_or_default()).into_owned()
}

fn process_failure_to_operational(
    failure: ProcessFailure,
    diagnostic_limit: usize,
) -> OperationalFailure {
    match failure {
        ProcessFailure::Spawn(error) => OperationalFailure {
            kind: OperationalFailureKind::ReferenceUnavailable,
            message: format!("could not start veraPDF: {error}"),
            diagnostics: None,
        },
        ProcessFailure::Wait(error) => OperationalFailure {
            kind: OperationalFailureKind::ReferenceProcess,
            message: format!("could not wait for veraPDF: {error}"),
            diagnostics: None,
        },
        ProcessFailure::Timeout(captured) => OperationalFailure {
            kind: OperationalFailureKind::Timeout,
            message: "veraPDF exceeded the configured timeout and was terminated".to_owned(),
            diagnostics: Some(diagnostics(&captured, diagnostic_limit)),
        },
        ProcessFailure::Read(stream, error) => OperationalFailure {
            kind: OperationalFailureKind::ReferenceProcess,
            message: format!("could not capture veraPDF {stream}: {error}"),
            diagnostics: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::ffi::OsStr;
    use std::process::Command;
    use std::time::Duration;

    use crate::{
        FailureCategory, ValidationCounts, ValidationFailure, ValidationProfile, ValidationReport,
    };

    use super::*;

    fn identity() -> ReferenceIdentity {
        ReferenceIdentity {
            product: "veraPDF",
            version: PINNED_VERAPDF_VERSION.to_owned(),
            profile: ReferenceProfile::PdfA1b,
        }
    }

    fn diagnostics_fixture() -> ReferenceDiagnostics {
        ReferenceDiagnostics {
            exit_code: Some(0),
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    fn local_report(passed: bool, category: FailureCategory) -> ValidationReport {
        ValidationReport {
            source: None,
            profile: ValidationProfile::PdfA1b,
            checks_passed: passed,
            preliminary: true,
            checks: ValidationCounts {
                total: 1,
                passed: usize::from(passed),
                failed: usize::from(!passed),
            },
            document: None,
            failures: if passed {
                Vec::new()
            } else {
                vec![ValidationFailure {
                    rule_id: if category == FailureCategory::Parser {
                        "PDF-PARSE-001".to_owned()
                    } else {
                        "PDFA1B-XMP-001".to_owned()
                    },
                    message: "fixture failure".to_owned(),
                    object_id: None,
                    category,
                }]
            },
        }
    }

    fn reference_result(
        compliant: Option<bool>,
        parse_state: ReferenceParseState,
    ) -> ReferenceResult {
        ReferenceResult {
            compliant,
            parse_state,
            profile_name: (parse_state == ReferenceParseState::Processed)
                .then(|| ReferenceProfile::PdfA1b.expected_profile_name().to_owned()),
            failed_rule_ids: Vec::new(),
            failed_rules: u64::from(compliant == Some(false)),
            failed_checks: u64::from(compliant == Some(false)),
            task_exception: None,
            diagnostics: diagnostics_fixture(),
        }
    }

    #[test]
    fn parses_noncompliant_reference_json_structurally() {
        let parsed = parse_reference_report(
            include_bytes!("../tests/reference-reports/noncompliant.json"),
            &identity(),
            diagnostics_fixture(),
        )
        .expect("reference report");
        assert_eq!(parsed.compliant, Some(false));
        assert_eq!(parsed.failed_rules, 1);
        assert_eq!(parsed.failed_checks, 2);
        assert_eq!(
            parsed.failed_rule_ids,
            vec![ReferenceRuleId {
                specification: "ISO 19005-1:2005".to_owned(),
                clause: "6.7.2".to_owned(),
                test_number: 1,
            }]
        );
    }

    #[test]
    fn parses_compliant_reference_json_structurally() {
        let parsed = parse_reference_report(
            include_bytes!("../tests/reference-reports/compliant.json"),
            &identity(),
            diagnostics_fixture(),
        )
        .expect("reference report");
        assert_eq!(parsed.compliant, Some(true));
        assert_eq!(parsed.parse_state, ReferenceParseState::Processed);
    }

    #[test]
    fn parses_one_ordered_result_per_batched_job() {
        let mut json: serde_json::Value =
            serde_json::from_slice(include_bytes!("../tests/reference-reports/compliant.json"))
                .expect("fixture JSON");
        let first_job = json["report"]["jobs"][0].clone();
        json["report"]["jobs"]
            .as_array_mut()
            .expect("jobs array")
            .push(first_job);
        let reports = parse_reference_reports(
            &serde_json::to_vec(&json).expect("serialize fixture JSON"),
            &identity(),
            diagnostics_fixture(),
            2,
        )
        .expect("batch report");
        assert_eq!(reports.len(), 2);
        assert!(reports.into_iter().all(|result| {
            result.is_ok_and(|report| report.parse_state == ReferenceParseState::Processed)
        }));
    }

    #[test]
    fn parses_reference_parse_exception_structurally() {
        let parsed = parse_reference_report(
            include_bytes!("../tests/reference-reports/parse-error.json"),
            &identity(),
            diagnostics_fixture(),
        )
        .expect("reference report");
        assert_eq!(parsed.compliant, None);
        assert_eq!(parsed.parse_state, ReferenceParseState::RejectedMalformed);
    }

    /// Confirmed against veraPDF 1.30.2: a page-tree node with a missing or
    /// unrecognized `/Type` surfaces as a `PARSE` task exception counted in
    /// `batchSummary.veraExceptions` rather than `failedParsingJobs`. This
    /// bucket must still classify as `RejectedMalformed`, not error out as
    /// an unrecognized reference report shape.
    #[test]
    fn parses_reference_parse_exception_reported_as_a_vera_exception() {
        let parsed = parse_reference_report(
            include_bytes!("../tests/reference-reports/parse-error-vera-exception.json"),
            &identity(),
            diagnostics_fixture(),
        )
        .expect("reference report");
        assert_eq!(parsed.compliant, None);
        assert_eq!(parsed.parse_state, ReferenceParseState::RejectedMalformed);
    }

    #[test]
    fn rejects_malformed_reference_report() {
        let error = parse_reference_report(b"{\"report\":", &identity(), diagnostics_fixture())
            .expect_err("malformed report");
        assert!(error.contains("invalid veraPDF JSON"));
    }

    #[test]
    fn classifies_all_semantic_outcomes() {
        let local_pass = local_report(true, FailureCategory::Conformance);
        let local_fail = local_report(false, FailureCategory::Conformance);
        let local_parse = local_report(false, FailureCategory::Parser);
        let reference_pass = reference_result(Some(true), ReferenceParseState::Processed);
        let reference_fail = reference_result(Some(false), ReferenceParseState::Processed);
        let reference_reject = reference_result(None, ReferenceParseState::RejectedMalformed);

        assert_eq!(
            classify(&local_pass, &reference_pass),
            ComparisonClassification::Agreement
        );
        assert_eq!(
            classify(&local_fail, &reference_fail),
            ComparisonClassification::BothNoncompliant
        );
        assert_eq!(
            classify(&local_pass, &reference_fail),
            ComparisonClassification::CoverageGap
        );
        assert_eq!(
            classify(&local_fail, &reference_pass),
            ComparisonClassification::LocalFalseNegative
        );
        assert_eq!(
            classify(&local_parse, &reference_pass),
            ComparisonClassification::LocalParserDiscrepancy
        );
        assert_eq!(
            classify(&local_pass, &reference_reject),
            ComparisonClassification::ReferenceParserDiscrepancy
        );
        assert_eq!(
            classify(&local_parse, &reference_reject),
            ComparisonClassification::BothNoncompliant
        );
    }

    #[test]
    fn classifications_have_expected_exit_code_classes() {
        assert_eq!(ComparisonClassification::Agreement.exit_code(), 0);
        assert_eq!(ComparisonClassification::BothNoncompliant.exit_code(), 0);
        assert_eq!(ComparisonClassification::CoverageGap.exit_code(), 0);
        assert_eq!(ComparisonClassification::LocalFalseNegative.exit_code(), 2);
        assert_eq!(
            ComparisonClassification::LocalParserDiscrepancy.exit_code(),
            2
        );
        assert_eq!(
            ComparisonClassification::ReferenceParserDiscrepancy.exit_code(),
            2
        );
        assert_eq!(ComparisonClassification::Operational.exit_code(), 1);
    }

    #[test]
    fn completed_profile_policy_rejects_coverage_gaps() {
        assert!(
            ComparisonClassification::CoverageGap
                .is_acceptable_under(CoverageGapPolicy::AllowDuringDevelopment)
        );
        assert!(
            !ComparisonClassification::CoverageGap
                .is_acceptable_under(CoverageGapPolicy::RejectForCompleteProfile)
        );
        assert!(
            ComparisonClassification::BothNoncompliant
                .is_acceptable_under(CoverageGapPolicy::RejectForCompleteProfile)
        );
    }

    #[test]
    fn recognizes_the_pinned_version_and_rejects_a_mismatch() {
        assert_eq!(
            serde_json::to_string(&ReferenceProfile::PdfA1b).expect("serialize profile"),
            "\"1b\""
        );
        assert_eq!(
            parse_version_output(b"veraPDF 1.30.2\nBuilt: fixture\n").expect("version"),
            "1.30.2"
        );
        let mismatch =
            verify_version("1.29.0", PINNED_VERAPDF_VERSION).expect_err("mismatch should fail");
        assert!(mismatch.contains("expected 1.30.2"));
        verify_cli_version("1.30.0", PINNED_VERAPDF_VERSION)
            .expect("1.30.2 distribution CLI banner reports apps 1.30.0");
    }

    #[test]
    fn missing_executable_is_reported() {
        let mut command = Command::new("definitely-missing-verapdf-executable-7ccda39b");
        let failure = run_command_capped(&mut command, Duration::from_secs(1), 1024)
            .expect_err("spawn must fail");
        assert!(matches!(failure, ProcessFailure::Spawn(_)));
    }

    #[test]
    fn controlled_child_is_terminated_on_timeout() {
        let executable = env::current_exe().expect("test executable");
        let mut command = Command::new(executable);
        command
            .args([
                "--exact",
                "differential::tests::fake_child_that_sleeps",
                "--nocapture",
            ])
            .env("PDF_DIFF_FAKE_CHILD_SLEEP", "1");
        let failure = run_command_capped(&mut command, Duration::from_millis(50), 1024)
            .expect_err("child should time out");
        let ProcessFailure::Timeout(captured) = failure else {
            panic!("expected timeout");
        };
        assert!(!captured.stdout.truncated);
    }

    #[test]
    fn fake_child_that_sleeps() {
        if env::var_os("PDF_DIFF_FAKE_CHILD_SLEEP").is_some() {
            thread::sleep(Duration::from_secs(2));
        }
    }

    #[test]
    fn pdf_path_with_spaces_is_one_process_argument() {
        let config = ReferenceConfig::pinned("verapdf");
        let command = build_validation_command(
            &config,
            &[Path::new("tests/fixtures/a directory/file name.pdf")],
        );
        let args = command.get_args().collect::<Vec<_>>();
        assert_eq!(
            args.last().copied(),
            Some(OsStr::new("tests/fixtures/a directory/file name.pdf"))
        );
    }

    #[test]
    fn batched_pdf_paths_are_distinct_process_arguments() {
        let config = ReferenceConfig::pinned("verapdf");
        let command = build_validation_command(
            &config,
            &[Path::new("first.pdf"), Path::new("second file.pdf")],
        );
        let args = command.get_args().collect::<Vec<_>>();
        assert_eq!(
            &args[args.len() - 2..],
            &[OsStr::new("first.pdf"), OsStr::new("second file.pdf")]
        );
    }
}
