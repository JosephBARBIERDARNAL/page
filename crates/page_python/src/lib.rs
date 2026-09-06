use std::path::PathBuf;

use page_validation::{
    FailureCategory as RustFailureCategory, PdfObjectId as RustPdfObjectId,
    SafetyLimits as RustSafetyLimits, ValidationCounts as RustValidationCounts,
    ValidationFailure as RustValidationFailure, ValidationProfile as RustValidationProfile,
    ValidationReport as RustValidationReport,
};
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;

create_exception!(_page, ValidationError, PyException);

#[pyclass(name = "ValidationProfile", frozen, eq, hash, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ValidationProfile {
    #[pyo3(name = "PDF_A_1B")]
    PdfA1b,
    #[pyo3(name = "PDF_A_1A")]
    PdfA1a,
    #[pyo3(name = "PDF_A_2B")]
    PdfA2b,
    #[pyo3(name = "PDF_A_2A")]
    PdfA2a,
    #[pyo3(name = "PDF_A_2U")]
    PdfA2u,
    #[pyo3(name = "PDF_A_3B")]
    PdfA3b,
    #[pyo3(name = "PDF_A_3A")]
    PdfA3a,
    #[pyo3(name = "PDF_A_3U")]
    PdfA3u,
    #[pyo3(name = "PDF_A_4")]
    PdfA4,
    #[pyo3(name = "PDF_A_4E")]
    PdfA4e,
    #[pyo3(name = "PDF_A_4F")]
    PdfA4f,
    #[pyo3(name = "PDF_UA_1")]
    PdfUa1,
    #[pyo3(name = "PDF_UA_2")]
    PdfUa2,
}

impl From<ValidationProfile> for RustValidationProfile {
    fn from(profile: ValidationProfile) -> Self {
        match profile {
            ValidationProfile::PdfA1b => Self::PdfA1b,
            ValidationProfile::PdfA1a => Self::PdfA1a,
            ValidationProfile::PdfA2b => Self::PdfA2b,
            ValidationProfile::PdfA2a => Self::PdfA2a,
            ValidationProfile::PdfA2u => Self::PdfA2u,
            ValidationProfile::PdfA3b => Self::PdfA3b,
            ValidationProfile::PdfA3a => Self::PdfA3a,
            ValidationProfile::PdfA3u => Self::PdfA3u,
            ValidationProfile::PdfA4 => Self::PdfA4,
            ValidationProfile::PdfA4e => Self::PdfA4e,
            ValidationProfile::PdfA4f => Self::PdfA4f,
            ValidationProfile::PdfUa1 => Self::PdfUa1,
            ValidationProfile::PdfUa2 => Self::PdfUa2,
        }
    }
}

impl From<RustValidationProfile> for ValidationProfile {
    fn from(profile: RustValidationProfile) -> Self {
        match profile {
            RustValidationProfile::PdfA1b => Self::PdfA1b,
            RustValidationProfile::PdfA1a => Self::PdfA1a,
            RustValidationProfile::PdfA2b => Self::PdfA2b,
            RustValidationProfile::PdfA2a => Self::PdfA2a,
            RustValidationProfile::PdfA2u => Self::PdfA2u,
            RustValidationProfile::PdfA3b => Self::PdfA3b,
            RustValidationProfile::PdfA3a => Self::PdfA3a,
            RustValidationProfile::PdfA3u => Self::PdfA3u,
            RustValidationProfile::PdfA4 => Self::PdfA4,
            RustValidationProfile::PdfA4e => Self::PdfA4e,
            RustValidationProfile::PdfA4f => Self::PdfA4f,
            RustValidationProfile::PdfUa1 => Self::PdfUa1,
            RustValidationProfile::PdfUa2 => Self::PdfUa2,
        }
    }
}

#[pyclass(name = "FailureCategory", frozen, eq, hash, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum FailureCategory {
    #[pyo3(name = "OPERATIONAL")]
    Operational,
    #[pyo3(name = "PARSER")]
    Parser,
    #[pyo3(name = "METADATA")]
    Metadata,
    #[pyo3(name = "CONFORMANCE")]
    Conformance,
}

impl From<RustFailureCategory> for FailureCategory {
    fn from(category: RustFailureCategory) -> Self {
        match category {
            RustFailureCategory::Operational => Self::Operational,
            RustFailureCategory::Parser => Self::Parser,
            RustFailureCategory::Metadata => Self::Metadata,
            RustFailureCategory::Conformance => Self::Conformance,
        }
    }
}

#[pyclass(name = "SafetyLimits", from_py_object)]
#[derive(Clone, Debug)]
struct SafetyLimits {
    #[pyo3(get, set)]
    max_input_size: u64,
    #[pyo3(get, set)]
    max_decoded_stream_size: usize,
    #[pyo3(get, set)]
    max_total_decoded_content_size: usize,
    #[pyo3(get, set)]
    max_object_count: usize,
    #[pyo3(get, set)]
    max_reference_depth: usize,
    #[pyo3(get, set)]
    max_xref_revisions: usize,
}

#[pymethods]
impl SafetyLimits {
    #[new]
    #[pyo3(signature = (*, max_input_size=None, max_decoded_stream_size=None, max_total_decoded_content_size=None, max_object_count=None, max_reference_depth=None, max_xref_revisions=None))]
    fn new(
        max_input_size: Option<u64>,
        max_decoded_stream_size: Option<usize>,
        max_total_decoded_content_size: Option<usize>,
        max_object_count: Option<usize>,
        max_reference_depth: Option<usize>,
        max_xref_revisions: Option<usize>,
    ) -> Self {
        let defaults = RustSafetyLimits::default();
        Self {
            max_input_size: max_input_size.unwrap_or(defaults.max_input_size),
            max_decoded_stream_size: max_decoded_stream_size
                .unwrap_or(defaults.max_decoded_stream_size),
            max_total_decoded_content_size: max_total_decoded_content_size
                .unwrap_or(defaults.max_total_decoded_content_size),
            max_object_count: max_object_count.unwrap_or(defaults.max_object_count),
            max_reference_depth: max_reference_depth.unwrap_or(defaults.max_reference_depth),
            max_xref_revisions: max_xref_revisions.unwrap_or(defaults.max_xref_revisions),
        }
    }

    #[classattr]
    const DEFAULT_MAX_INPUT_SIZE: u64 = RustSafetyLimits::DEFAULT_MAX_INPUT_SIZE;

    #[classattr]
    const DEFAULT_MAX_DECODED_STREAM_SIZE: usize =
        RustSafetyLimits::DEFAULT_MAX_DECODED_STREAM_SIZE;

    #[classattr]
    const DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE: usize =
        RustSafetyLimits::DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE;

    #[classattr]
    const DEFAULT_MAX_OBJECT_COUNT: usize = RustSafetyLimits::DEFAULT_MAX_OBJECT_COUNT;

    #[classattr]
    const DEFAULT_MAX_REFERENCE_DEPTH: usize = RustSafetyLimits::DEFAULT_MAX_REFERENCE_DEPTH;

    #[classattr]
    const DEFAULT_MAX_XREF_REVISIONS: usize = RustSafetyLimits::DEFAULT_MAX_XREF_REVISIONS;

    fn __repr__(&self) -> String {
        format!(
            "SafetyLimits(max_input_size={}, max_decoded_stream_size={}, max_total_decoded_content_size={}, max_object_count={}, max_reference_depth={}, max_xref_revisions={})",
            self.max_input_size,
            self.max_decoded_stream_size,
            self.max_total_decoded_content_size,
            self.max_object_count,
            self.max_reference_depth,
            self.max_xref_revisions,
        )
    }
}

impl From<&SafetyLimits> for RustSafetyLimits {
    fn from(limits: &SafetyLimits) -> Self {
        Self {
            max_input_size: limits.max_input_size,
            max_decoded_stream_size: limits.max_decoded_stream_size,
            max_total_decoded_content_size: limits.max_total_decoded_content_size,
            max_object_count: limits.max_object_count,
            max_reference_depth: limits.max_reference_depth,
            max_xref_revisions: limits.max_xref_revisions,
        }
    }
}

#[pyclass(name = "PdfObjectId", frozen, from_py_object)]
#[derive(Clone)]
struct PdfObjectId {
    #[pyo3(get)]
    object_number: u32,
    #[pyo3(get)]
    generation: u16,
}

impl From<RustPdfObjectId> for PdfObjectId {
    fn from(id: RustPdfObjectId) -> Self {
        Self {
            object_number: id.object_number,
            generation: id.generation,
        }
    }
}

#[pymethods]
impl PdfObjectId {
    fn __repr__(&self) -> String {
        format!("PdfObjectId({}, {})", self.object_number, self.generation)
    }
}

#[pyclass(name = "ValidationFailure", frozen, from_py_object)]
#[derive(Clone)]
struct ValidationFailure {
    inner: RustValidationFailure,
}

impl From<RustValidationFailure> for ValidationFailure {
    fn from(inner: RustValidationFailure) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl ValidationFailure {
    #[getter]
    fn rule_id(&self) -> &str {
        &self.inner.rule_id
    }

    #[getter]
    fn message(&self) -> &str {
        &self.inner.message
    }

    #[getter]
    fn object_id(&self) -> Option<PdfObjectId> {
        self.inner.object_id.map(Into::into)
    }

    #[getter]
    fn category(&self) -> FailureCategory {
        self.inner.category.into()
    }

    fn __repr__(&self) -> String {
        format!(
            "ValidationFailure(rule_id={:?}, category={:?}, message={:?})",
            self.inner.rule_id,
            FailureCategory::from(self.inner.category),
            self.inner.message,
        )
    }
}

#[pyclass(name = "ValidationCounts", frozen, from_py_object)]
#[derive(Clone)]
struct ValidationCounts {
    #[pyo3(get)]
    total: usize,
    #[pyo3(get)]
    passed: usize,
    #[pyo3(get)]
    failed: usize,
}

impl From<RustValidationCounts> for ValidationCounts {
    fn from(counts: RustValidationCounts) -> Self {
        Self {
            total: counts.total,
            passed: counts.passed,
            failed: counts.failed,
        }
    }
}

#[pymethods]
impl ValidationCounts {
    fn __repr__(&self) -> String {
        format!(
            "ValidationCounts(total={}, passed={}, failed={})",
            self.total, self.passed, self.failed
        )
    }
}

#[pyclass(name = "PdfDocument", frozen, from_py_object)]
#[derive(Clone)]
struct PdfDocument {
    #[pyo3(get)]
    version: String,
    #[pyo3(get)]
    encrypted: bool,
    #[pyo3(get)]
    page_count: usize,
    #[pyo3(get)]
    object_count: usize,
}

#[pymethods]
impl PdfDocument {
    fn __repr__(&self) -> String {
        format!(
            "PdfDocument(version={:?}, encrypted={}, page_count={}, object_count={})",
            self.version, self.encrypted, self.page_count, self.object_count
        )
    }
}

#[pyclass(name = "ValidationReport", frozen)]
struct ValidationReport {
    inner: RustValidationReport,
}

impl From<RustValidationReport> for ValidationReport {
    fn from(inner: RustValidationReport) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl ValidationReport {
    #[getter]
    fn profile(&self) -> ValidationProfile {
        self.inner.profile.into()
    }

    #[getter]
    fn is_compliant(&self) -> bool {
        self.inner.is_compliant
    }

    #[getter]
    fn preliminary(&self) -> bool {
        self.inner.preliminary
    }

    #[getter]
    fn checks(&self) -> ValidationCounts {
        self.inner.checks.into()
    }

    #[getter]
    fn document(&self) -> Option<PdfDocument> {
        self.inner.document.as_ref().map(|document| PdfDocument {
            version: document.version.clone(),
            encrypted: document.encrypted,
            page_count: document.page_count,
            object_count: document.object_count,
        })
    }

    #[getter]
    fn failures(&self) -> Vec<ValidationFailure> {
        self.inner
            .failures
            .iter()
            .cloned()
            .map(Into::into)
            .collect()
    }

    fn has_operational_failure(&self) -> bool {
        self.inner.has_operational_failure()
    }

    fn exit_code(&self) -> i32 {
        self.inner.exit_code()
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.json_report())
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "ValidationReport(profile={:?}, is_compliant={}, failures={})",
            ValidationProfile::from(self.inner.profile),
            self.inner.is_compliant,
            self.inner.failures.len(),
        )
    }
}

fn limits_or_default(limits: Option<&SafetyLimits>) -> RustSafetyLimits {
    limits.map(Into::into).unwrap_or_default()
}

#[pyfunction]
#[pyo3(signature = (path, profile=None, limits=None))]
fn is_pdf_compliant(
    py: Python<'_>,
    path: PathBuf,
    profile: Option<ValidationProfile>,
    limits: Option<&SafetyLimits>,
) -> PyResult<bool> {
    let limits = limits_or_default(limits);
    py.detach(|| page_validation::is_pdf_compliant(&path, profile.map(Into::into), &limits))
        .map_err(|error| ValidationError::new_err(error.to_string()))
}

#[pyfunction]
#[pyo3(signature = (path, profile=None, limits=None))]
fn validate_pdf(
    py: Python<'_>,
    path: PathBuf,
    profile: Option<ValidationProfile>,
    limits: Option<&SafetyLimits>,
) -> PyResult<ValidationReport> {
    let limits = limits_or_default(limits);
    py.detach(|| page_validation::validate_pdf(&path, profile.map(Into::into), &limits))
        .map(Into::into)
        .map_err(|error| ValidationError::new_err(error.to_string()))
}

#[pyfunction]
#[pyo3(signature = (data, profile=None, limits=None))]
fn is_pdf_compliant_bytes(
    py: Python<'_>,
    data: &[u8],
    profile: Option<ValidationProfile>,
    limits: Option<&SafetyLimits>,
) -> PyResult<bool> {
    let data = data.to_vec();
    let limits = limits_or_default(limits);
    py.detach(|| page_validation::is_pdf_compliant_bytes(&data, profile.map(Into::into), &limits))
        .map_err(|error| ValidationError::new_err(error.to_string()))
}

#[pyfunction]
#[pyo3(signature = (data, profile=None, limits=None))]
fn validate_pdf_bytes(
    py: Python<'_>,
    data: &[u8],
    profile: Option<ValidationProfile>,
    limits: Option<&SafetyLimits>,
) -> PyResult<ValidationReport> {
    let data = data.to_vec();
    let limits = limits_or_default(limits);
    py.detach(|| page_validation::validate_pdf_bytes(&data, profile.map(Into::into), &limits))
        .map(Into::into)
        .map_err(|error| ValidationError::new_err(error.to_string()))
}

#[pymodule]
fn _page(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("ValidationError", py.get_type::<ValidationError>())?;
    module.add_class::<ValidationProfile>()?;
    module.add_class::<FailureCategory>()?;
    module.add_class::<SafetyLimits>()?;
    module.add_class::<PdfObjectId>()?;
    module.add_class::<ValidationFailure>()?;
    module.add_class::<ValidationCounts>()?;
    module.add_class::<PdfDocument>()?;
    module.add_class::<ValidationReport>()?;
    module.add_function(wrap_pyfunction!(is_pdf_compliant, module)?)?;
    module.add_function(wrap_pyfunction!(is_pdf_compliant_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(validate_pdf, module)?)?;
    module.add_function(wrap_pyfunction!(validate_pdf_bytes, module)?)?;
    Ok(())
}
