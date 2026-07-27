//! Differential validation against the pinned veraPDF reference.
//!
//! This module compares the crate's deliberately incomplete PDF/A-1b checks
//! with veraPDF. It does not turn the local validator into a complete
//! conformance checker.

use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::str::FromStr;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

use crate::{FailureCategory, SafetyLimits, ValidationProfile, ValidationReport, validate_file};

pub const PINNED_VERAPDF_VERSION: &str = "1.28.2";
pub const PINNED_VERAPDF_PROFILE: ReferenceProfile = ReferenceProfile::PdfA1b;
pub const DEFAULT_TIMEOUT_MILLIS: u64 = 30_000;
pub const DEFAULT_MAX_REPORT_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ReferenceProfile {
    #[serde(rename = "1b")]
    PdfA1b,
}

impl ReferenceProfile {
    pub const fn as_verapdf_flavour(self) -> &'static str {
        match self {
            Self::PdfA1b => "1b",
        }
    }

    const fn expected_profile_name(self) -> &'static str {
        match self {
            Self::PdfA1b => "PDF/A-1B validation profile",
        }
    }
}

impl fmt::Display for ReferenceProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_verapdf_flavour())
    }
}

impl FromStr for ReferenceProfile {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "1b" => Ok(Self::PdfA1b),
            _ => Err(format!(
                "unsupported reference profile {value:?}; only 1b is implemented"
            )),
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
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReferenceIdentity {
    pub product: &'static str,
    pub version: String,
    pub profile: ReferenceProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingStrength {
    Exact,
    PartialProxy,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct RuleMapping {
    pub local_rule_id: &'static str,
    pub verapdf_rule_id: Option<&'static str>,
    pub iso_clause: Option<&'static str>,
    pub strength: MappingStrength,
    pub reference_test: Option<&'static str>,
    pub notes: &'static str,
}

pub const RULE_MAPPINGS: [RuleMapping; 18] = [
    RuleMapping {
        local_rule_id: "PDF-PARSE-001",
        verapdf_rule_id: None,
        iso_clause: None,
        strength: MappingStrength::None,
        reference_test: None,
        notes: "Operational parser gate; it is not an ISO conformance rule.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-ENCRYPTION-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.1.3:2"),
        iso_clause: Some("ISO 19005-1:2005, 6.1.3"),
        strength: MappingStrength::Exact,
        reference_test: Some("isEncrypted != true"),
        notes: "Both checks reject an Encrypt entry in the trailer.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-CATALOG-001",
        verapdf_rule_id: None,
        iso_clause: None,
        strength: MappingStrength::None,
        reference_test: None,
        notes: "Foundational local object-model gate; the pinned profile has no standalone catalog-exists rule.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-XMP-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.9:1"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.9"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("isSerializationValid"),
        notes: "Well-formed, bounded XML is necessary but not sufficient for XMP 2004 serialization and extension-schema validity.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-METADATA-STRUCTURE-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.2:1"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.2"),
        strength: MappingStrength::Exact,
        reference_test: Some("containsMetadata == true"),
        notes: "Requires catalog Metadata to resolve to a stream with Type /Metadata and Subtype /XML.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-METADATA-FILTER-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.2:2"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.2"),
        strength: MappingStrength::Exact,
        reference_test: Some("isCatalogMetadata == false || Filter == null"),
        notes: "The predicate is evaluated only for the catalog Metadata stream.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-ID-SCHEMA-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.11:1"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.11"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("containsPDFAIdentification == true"),
        notes: "The local bounded XML model detects namespace properties but does not implement XMP 2004 package recovery; invalid duplicate packages can be classified differently.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-ID-PART-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.11:2"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.11"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("part == 1"),
        notes: "The common single-property case matches; duplicate or invalid XMP packages can be represented differently by veraPDF's XMP model.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-ID-CONFORMANCE-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.11:3"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.11"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("conformance == \"B\" || conformance == \"A\""),
        notes: "The common single-property case matches and accepts A or B; duplicate or invalid XMP packages can be represented differently by veraPDF's XMP model.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-INFO-CREATIONDATE-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.3:1"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.3"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("doCreationDatesMatch != false"),
        notes: "Common full PDF and ISO-8601 dates are compared as instants; uncommon reduced-precision XMP forms remain unsupported.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-INFO-TITLE-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.3:2"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.3"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("Title == null || Title == XMPTitle"),
        notes: "ASCII RDF Alt cases match; complete PDFDocEncoding and the full XMP 2004 data model are not implemented.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-INFO-AUTHOR-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.3:3"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.3"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("Author == null || (Author == XMPCreator && XMPCreatorSize == 1)"),
        notes: "ASCII RDF Seq cases and multiplicity match; complete PDFDocEncoding and the full XMP 2004 data model are not implemented.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-INFO-SUBJECT-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.3:4"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.3"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("Subject == null || Subject == XMPDescription"),
        notes: "ASCII RDF Alt cases match; complete PDFDocEncoding and the full XMP 2004 data model are not implemented.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-INFO-KEYWORDS-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.3:5"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.3"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("Keywords == null || Keywords == XMPKeywords"),
        notes: "ASCII scalar cases match; complete PDFDocEncoding and the full XMP 2004 data model are not implemented.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-INFO-CREATOR-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.3:6"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.3"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("Creator == null || Creator == XMPCreatorTool"),
        notes: "ASCII scalar cases match; complete PDFDocEncoding and the full XMP 2004 data model are not implemented.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-INFO-PRODUCER-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.3:7"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.3"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("Producer == null || Producer == XMPProducer"),
        notes: "ASCII scalar cases match; complete PDFDocEncoding and the full XMP 2004 data model are not implemented.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-INFO-MODDATE-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.7.3:8"),
        iso_clause: Some("ISO 19005-1:2005, 6.7.3"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some("doModDatesMatch != false"),
        notes: "Common full PDF and ISO-8601 dates are compared as instants; uncommon reduced-precision XMP forms remain unsupported.",
    },
    RuleMapping {
        local_rule_id: "PDFA1B-OUTPUTINTENT-001",
        verapdf_rule_id: Some("ISO 19005-1:2005:6.2.2:1"),
        iso_clause: Some("ISO 19005-1:2005, 6.2.2"),
        strength: MappingStrength::PartialProxy,
        reference_test: Some(
            "(deviceClass == \"prtr\" || deviceClass == \"mntr\") && (colorSpace == \"RGB \" || colorSpace == \"CMYK\" || colorSpace == \"GRAY\") && version < 3.0",
        ),
        notes: "A nonempty OutputIntents array is only a local proxy; it does not validate S, DestOutputProfile, ICC class, colour space, version, or BToA data.",
    },
];

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
    pub identity: ReferenceIdentity,
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
        self.classification.exit_code()
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
            writeln!(
                formatter,
                "Operational failure ({:?}): {}",
                failure.kind, failure.message
            )?;
        }
        writeln!(
            formatter,
            "Local implemented checks passed: {}",
            self.local_report.implemented_checks_passed
        )?;
        if self.classification == ComparisonClassification::CoverageGap {
            writeln!(
                formatter,
                "Warning: coverage_gap means only the local subset passed; the PDF is not PDF/A-1b compliant."
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
        verify_version(&actual_version, &config.expected_version).map_err(|message| {
            OperationalFailure {
                kind: OperationalFailureKind::VersionMismatch,
                message,
                diagnostics: Some(diagnostics),
            }
        })?;
        let identity = ReferenceIdentity {
            product: "veraPDF",
            version: actual_version,
            profile: config.profile,
        };
        Ok(Self { config, identity })
    }

    pub fn identity(&self) -> &ReferenceIdentity {
        &self.identity
    }

    pub fn compare_file(&self, path: &Path, limits: &SafetyLimits) -> DifferentialReport {
        let local_report = validate_file(path, ValidationProfile::PdfA1b, limits);
        if local_report
            .failures
            .iter()
            .any(|failure| failure.category == FailureCategory::Operational)
        {
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
                    acceptable: classification.is_acceptable(),
                    summary: classification_summary(classification).to_owned(),
                    local_report,
                    reference_result: Some(reference_result),
                    operational_failure: None,
                }
            }
            Err(failure) => self.operational_report(path, local_report, failure),
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
        let mut command = build_validation_command(&self.config, path);
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
        parse_reference_report(
            &captured.stdout.bytes,
            &self.identity,
            report_diagnostics.clone(),
        )
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
                match (reference.compliant, local.implemented_checks_passed) {
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

fn build_validation_command(config: &ReferenceConfig, path: &Path) -> Command {
    let mut command = Command::new(&config.executable);
    command.args([
        "--loglevel",
        "0",
        "--format",
        "json",
        "--flavour",
        config.profile.as_verapdf_flavour(),
    ]);
    command.arg(path);
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
}

fn parse_reference_report(
    bytes: &[u8],
    expected_identity: &ReferenceIdentity,
    diagnostics: ReferenceDiagnostics,
) -> Result<ReferenceResult, String> {
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
    if raw.report.jobs.len() != 1 {
        return Err(format!(
            "expected exactly one veraPDF job, found {}",
            raw.report.jobs.len()
        ));
    }
    let job = raw.report.jobs.into_iter().next().expect("length checked");
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
                identity: expected_identity.clone(),
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
            let parse_state = if raw.report.batch_summary.failed_encrypted_jobs > 0 {
                ReferenceParseState::RejectedEncrypted
            } else if raw.report.batch_summary.failed_parsing_jobs > 0 {
                ReferenceParseState::RejectedMalformed
            } else {
                return Err(
                    "veraPDF PARSE exception was not reflected in the batch summary".to_owned(),
                );
            };
            Ok(ReferenceResult {
                identity: expected_identity.clone(),
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
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            return Err(ProcessFailure::Wait(error));
        }
    };
    let status = match waited {
        Some(status) => status,
        None => {
            let _ = child.kill();
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
        .map_err(|_| ProcessFailure::Read("stdout", "reader thread panicked".to_owned()))?
        .map_err(|error| ProcessFailure::Read("stdout", error.to_string()))?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| ProcessFailure::Read("stderr", "reader thread panicked".to_owned()))?
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
        bytes.extend_from_slice(&buffer[..retained]);
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
    String::from_utf8_lossy(&bytes[..bytes.len().min(limit)]).into_owned()
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
            profile: ValidationProfile::PdfA1b,
            implemented_checks_passed: passed,
            preliminary: true,
            disclaimer: "This report covers only preliminary checks and does not establish full PDF/A compliance.",
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
                        "PDF-PARSE-001"
                    } else {
                        "PDFA1B-XMP-001"
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
            identity: identity(),
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
    fn recognizes_the_pinned_version_and_rejects_a_mismatch() {
        assert_eq!(
            serde_json::to_string(&ReferenceProfile::PdfA1b).expect("serialize profile"),
            "\"1b\""
        );
        assert_eq!(
            parse_version_output(b"veraPDF 1.28.2\nBuilt: fixture\n").expect("version"),
            "1.28.2"
        );
        let mismatch =
            verify_version("1.29.0", PINNED_VERAPDF_VERSION).expect_err("mismatch should fail");
        assert!(mismatch.contains("expected 1.28.2"));
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
            Path::new("tests/fixtures/a directory/file name.pdf"),
        );
        let args = command.get_args().collect::<Vec<_>>();
        assert_eq!(
            args.last().copied(),
            Some(OsStr::new("tests/fixtures/a directory/file name.pdf"))
        );
    }
}
