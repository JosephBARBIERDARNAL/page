use std::path::PathBuf;

use page_validation::{
    FailureCategory as RustFailureCategory, PageError as RustPageError,
    PdfObjectId as RustPdfObjectId, SafetyLimits as RustSafetyLimits,
    ValidationCheckCounts as RustValidationCheckCounts, ValidationCounts as RustValidationCounts,
    ValidationErrorKind as RustValidationErrorKind, ValidationFailure as RustValidationFailure,
    ValidationProfile as RustValidationProfile, ValidationReport as RustValidationReport,
};
use pyo3::buffer::PyBuffer;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyFileNotFoundError, PyOSError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedBytes;
use pyo3::types::{PyBytes, PyMemoryView, PyString};

create_exception!(_page, PageError, PyException);
create_exception!(_page, ParseError, PageError);
create_exception!(_page, SafetyLimitError, PageError);
create_exception!(_page, ProfileError, PageError);

fn python_validation_error(error: RustPageError) -> PyErr {
    match error {
        RustPageError::InputIo(error) => {
            if error.kind() == std::io::ErrorKind::NotFound {
                PyFileNotFoundError::new_err(error.to_string())
            } else {
                PyOSError::new_err(error.to_string())
            }
        }
        error => {
            let message = error.to_string();
            match error.kind() {
                RustValidationErrorKind::Parser => ParseError::new_err(message),
                RustValidationErrorKind::SafetyLimit => SafetyLimitError::new_err(message),
                RustValidationErrorKind::Profile => ProfileError::new_err(message),
                RustValidationErrorKind::InputIo => PageError::new_err(message),
                _ => PageError::new_err(message),
            }
        }
    }
}

fn python_enum_value(py: Python<'_>, enum_name: &str, value: &str) -> PyResult<Py<PyAny>> {
    let enum_module = py.import("page._enums")?;
    Ok(enum_module.getattr(enum_name)?.call1((value,))?.unbind())
}

fn parse_validation_profile(
    py: Python<'_>,
    profile: &Bound<'_, PyAny>,
) -> PyResult<RustValidationProfile> {
    let profile_enum = py.import("page._enums")?.getattr("ValidationProfile")?;
    let value = if profile.is_instance_of::<PyString>() {
        profile.extract::<String>()?
    } else if profile.is_instance(&profile_enum)? {
        profile.getattr("value")?.extract::<String>()?
    } else {
        return Err(PyTypeError::new_err(
            "profile must be a ValidationProfile, a string, or None",
        ));
    };

    value
        .parse()
        .map_err(|error: page_validation::ParseValidationProfileError| {
            PyValueError::new_err(error.to_string())
        })
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
    max_form_invocations: usize,
    #[pyo3(get, set)]
    max_object_count: usize,
    #[pyo3(get, set)]
    max_reference_depth: usize,
    #[pyo3(get, set)]
    max_xref_revisions: usize,
    #[pyo3(get, set)]
    max_table_span: usize,
    #[pyo3(get, set)]
    max_table_grid_rows: usize,
    #[pyo3(get, set)]
    max_table_grid_columns: usize,
    #[pyo3(get, set)]
    max_table_grid_cells: usize,
    #[pyo3(get, set)]
    max_unicode_cmap_mappings: usize,
}

#[pymethods]
impl SafetyLimits {
    #[new]
    #[pyo3(signature = (*, max_input_size=None, max_decoded_stream_size=None, max_total_decoded_content_size=None, max_form_invocations=None, max_object_count=None, max_reference_depth=None, max_xref_revisions=None, max_table_span=None, max_table_grid_rows=None, max_table_grid_columns=None, max_table_grid_cells=None, max_unicode_cmap_mappings=None))]
    fn new(
        max_input_size: Option<u64>,
        max_decoded_stream_size: Option<usize>,
        max_total_decoded_content_size: Option<usize>,
        max_form_invocations: Option<usize>,
        max_object_count: Option<usize>,
        max_reference_depth: Option<usize>,
        max_xref_revisions: Option<usize>,
        max_table_span: Option<usize>,
        max_table_grid_rows: Option<usize>,
        max_table_grid_columns: Option<usize>,
        max_table_grid_cells: Option<usize>,
        max_unicode_cmap_mappings: Option<usize>,
    ) -> Self {
        let defaults = RustSafetyLimits::default();
        Self {
            max_input_size: max_input_size.unwrap_or(defaults.max_input_size()),
            max_decoded_stream_size: max_decoded_stream_size
                .unwrap_or(defaults.max_decoded_stream_size()),
            max_total_decoded_content_size: max_total_decoded_content_size
                .unwrap_or(defaults.max_total_decoded_content_size()),
            max_form_invocations: max_form_invocations.unwrap_or(defaults.max_form_invocations()),
            max_object_count: max_object_count.unwrap_or(defaults.max_object_count()),
            max_reference_depth: max_reference_depth.unwrap_or(defaults.max_reference_depth()),
            max_xref_revisions: max_xref_revisions.unwrap_or(defaults.max_xref_revisions()),
            max_table_span: max_table_span.unwrap_or(defaults.max_table_span()),
            max_table_grid_rows: max_table_grid_rows.unwrap_or(defaults.max_table_grid_rows()),
            max_table_grid_columns: max_table_grid_columns
                .unwrap_or(defaults.max_table_grid_columns()),
            max_table_grid_cells: max_table_grid_cells.unwrap_or(defaults.max_table_grid_cells()),
            max_unicode_cmap_mappings: max_unicode_cmap_mappings
                .unwrap_or(defaults.max_unicode_cmap_mappings()),
        }
    }

    /// Disable all configurable safety limits. Use only with trusted files.
    #[staticmethod]
    fn unlimited() -> Self {
        RustSafetyLimits::unlimited().into()
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
    const DEFAULT_MAX_FORM_INVOCATIONS: usize = RustSafetyLimits::DEFAULT_MAX_FORM_INVOCATIONS;

    #[classattr]
    const DEFAULT_MAX_OBJECT_COUNT: usize = RustSafetyLimits::DEFAULT_MAX_OBJECT_COUNT;

    #[classattr]
    const DEFAULT_MAX_REFERENCE_DEPTH: usize = RustSafetyLimits::DEFAULT_MAX_REFERENCE_DEPTH;

    #[classattr]
    const DEFAULT_MAX_XREF_REVISIONS: usize = RustSafetyLimits::DEFAULT_MAX_XREF_REVISIONS;

    #[classattr]
    const DEFAULT_MAX_TABLE_SPAN: usize = RustSafetyLimits::DEFAULT_MAX_TABLE_SPAN;

    #[classattr]
    const DEFAULT_MAX_TABLE_GRID_ROWS: usize = RustSafetyLimits::DEFAULT_MAX_TABLE_GRID_ROWS;

    #[classattr]
    const DEFAULT_MAX_TABLE_GRID_COLUMNS: usize = RustSafetyLimits::DEFAULT_MAX_TABLE_GRID_COLUMNS;

    #[classattr]
    const DEFAULT_MAX_TABLE_GRID_CELLS: usize = RustSafetyLimits::DEFAULT_MAX_TABLE_GRID_CELLS;

    #[classattr]
    const DEFAULT_MAX_UNICODE_CMAP_MAPPINGS: usize =
        RustSafetyLimits::DEFAULT_MAX_UNICODE_CMAP_MAPPINGS;

    fn __repr__(&self) -> String {
        format!(
            "SafetyLimits(max_input_size={}, max_decoded_stream_size={}, max_total_decoded_content_size={}, max_form_invocations={}, max_object_count={}, max_reference_depth={}, max_xref_revisions={}, max_table_span={}, max_table_grid_rows={}, max_table_grid_columns={}, max_table_grid_cells={}, max_unicode_cmap_mappings={})",
            self.max_input_size,
            self.max_decoded_stream_size,
            self.max_total_decoded_content_size,
            self.max_form_invocations,
            self.max_object_count,
            self.max_reference_depth,
            self.max_xref_revisions,
            self.max_table_span,
            self.max_table_grid_rows,
            self.max_table_grid_columns,
            self.max_table_grid_cells,
            self.max_unicode_cmap_mappings,
        )
    }
}

impl From<RustSafetyLimits> for SafetyLimits {
    fn from(limits: RustSafetyLimits) -> Self {
        Self {
            max_input_size: limits.max_input_size(),
            max_decoded_stream_size: limits.max_decoded_stream_size(),
            max_total_decoded_content_size: limits.max_total_decoded_content_size(),
            max_form_invocations: limits.max_form_invocations(),
            max_object_count: limits.max_object_count(),
            max_reference_depth: limits.max_reference_depth(),
            max_xref_revisions: limits.max_xref_revisions(),
            max_table_span: limits.max_table_span(),
            max_table_grid_rows: limits.max_table_grid_rows(),
            max_table_grid_columns: limits.max_table_grid_columns(),
            max_table_grid_cells: limits.max_table_grid_cells(),
            max_unicode_cmap_mappings: limits.max_unicode_cmap_mappings(),
        }
    }
}

impl From<&SafetyLimits> for RustSafetyLimits {
    fn from(limits: &SafetyLimits) -> Self {
        Self::default()
            .with_max_input_size(limits.max_input_size)
            .with_max_decoded_stream_size(limits.max_decoded_stream_size)
            .with_max_total_decoded_content_size(limits.max_total_decoded_content_size)
            .with_max_form_invocations(limits.max_form_invocations)
            .with_max_object_count(limits.max_object_count)
            .with_max_reference_depth(limits.max_reference_depth)
            .with_max_xref_revisions(limits.max_xref_revisions)
            .with_max_table_span(limits.max_table_span)
            .with_max_table_grid_rows(limits.max_table_grid_rows)
            .with_max_table_grid_columns(limits.max_table_grid_columns)
            .with_max_table_grid_cells(limits.max_table_grid_cells)
            .with_max_unicode_cmap_mappings(limits.max_unicode_cmap_mappings)
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
    fn category(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = match self.inner.category {
            RustFailureCategory::Metadata => "metadata",
            RustFailureCategory::Conformance => "conformance",
            _ => {
                return Err(PageError::new_err(format!(
                    "failure category {:?} is not supported by the Python bindings",
                    self.inner.category
                )));
            }
        };
        python_enum_value(py, "FailureCategory", value)
    }

    fn __repr__(&self) -> String {
        format!(
            "ValidationFailure(rule_id={:?}, category={:?}, message={:?})",
            self.inner.rule_id, self.inner.category, self.inner.message,
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

#[pyclass(name = "ValidationCheckCounts", frozen, from_py_object)]
#[derive(Clone)]
struct ValidationCheckCounts {
    #[pyo3(get)]
    failed: usize,
}

impl From<RustValidationCheckCounts> for ValidationCheckCounts {
    fn from(counts: RustValidationCheckCounts) -> Self {
        Self {
            failed: counts.failed,
        }
    }
}

#[pymethods]
impl ValidationCheckCounts {
    fn __repr__(&self) -> String {
        format!("ValidationCheckCounts(failed={})", self.failed)
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
    fn source(&self) -> Option<String> {
        self.inner
            .source
            .as_ref()
            .map(|source| source.display().to_string())
    }

    #[getter]
    fn profile(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        python_enum_value(py, "ValidationProfile", self.inner.profile.as_str())
    }

    #[getter]
    fn is_compliant(&self) -> bool {
        self.inner.is_compliant
    }

    #[getter]
    fn rules(&self) -> ValidationCounts {
        self.inner.rules.into()
    }

    #[getter]
    fn checks(&self) -> ValidationCheckCounts {
        self.inner.checks.into()
    }

    #[getter]
    fn document(&self) -> PdfDocument {
        let document = &self.inner.document;
        PdfDocument {
            version: document.version.clone(),
            encrypted: document.encrypted,
            page_count: document.page_count,
            object_count: document.object_count,
        }
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

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.json_report())
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let json = self.to_json()?;
        Ok(py.import("json")?.call_method1("loads", (json,))?.unbind())
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "ValidationReport(profile={:?}, is_compliant={}, failures={})",
            self.inner.profile.as_str(),
            self.inner.is_compliant,
            self.inner.failures.len(),
        )
    }
}

fn validation_options(
    py: Python<'_>,
    profile: Option<&Bound<'_, PyAny>>,
    limits: Option<&SafetyLimits>,
) -> PyResult<page_validation::ValidationOptions> {
    let profile = profile
        .map(|profile| parse_validation_profile(py, profile))
        .transpose()?;
    Ok(page_validation::ValidationOptions::default()
        .profile(profile)
        .limits(limits.map(Into::into).unwrap_or_default()))
}

enum PythonPdfBytes {
    Backed(PyBackedBytes),
    Owned(Vec<u8>),
}

impl AsRef<[u8]> for PythonPdfBytes {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Backed(data) => data.as_ref(),
            Self::Owned(data) => data,
        }
    }
}

fn python_pdf_bytes(data: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PythonPdfBytes> {
    if let Ok(bytes) = data.cast::<PyBytes>() {
        return Ok(PythonPdfBytes::Backed(PyBackedBytes::from(
            bytes.to_owned(),
        )));
    }

    let normalized;
    let data = if data.is_instance_of::<PyMemoryView>() {
        normalized = data.call_method0("tobytes")?;
        &normalized
    } else {
        data
    };
    let buffer = PyBuffer::<u8>::get(data)?;
    Ok(PythonPdfBytes::Owned(buffer.to_vec(py)?))
}

#[pyfunction]
#[pyo3(signature = (path, *, profile=None, limits=None))]
fn is_pdf_compliant(
    py: Python<'_>,
    path: PathBuf,
    profile: Option<&Bound<'_, PyAny>>,
    limits: Option<&SafetyLimits>,
) -> PyResult<bool> {
    let options = validation_options(py, profile, limits)?;
    py.detach(|| page_validation::is_pdf_compliant(&path, &options))
        .map_err(python_validation_error)
}

#[pyfunction]
#[pyo3(signature = (path, *, profile=None, limits=None))]
fn validate_pdf(
    py: Python<'_>,
    path: PathBuf,
    profile: Option<&Bound<'_, PyAny>>,
    limits: Option<&SafetyLimits>,
) -> PyResult<ValidationReport> {
    let options = validation_options(py, profile, limits)?;
    py.detach(|| page_validation::validate_pdf(&path, &options))
        .map(Into::into)
        .map_err(python_validation_error)
}

#[pyfunction]
#[pyo3(signature = (data, *, profile=None, limits=None))]
fn is_pdf_compliant_bytes(
    py: Python<'_>,
    data: Bound<'_, PyAny>,
    profile: Option<&Bound<'_, PyAny>>,
    limits: Option<&SafetyLimits>,
) -> PyResult<bool> {
    let data = python_pdf_bytes(&data, py)?;
    let options = validation_options(py, profile, limits)?;
    py.detach(|| page_validation::is_pdf_compliant_bytes(data.as_ref(), &options))
        .map_err(python_validation_error)
}

#[pyfunction]
#[pyo3(signature = (data, *, profile=None, limits=None))]
fn validate_pdf_bytes(
    py: Python<'_>,
    data: Bound<'_, PyAny>,
    profile: Option<&Bound<'_, PyAny>>,
    limits: Option<&SafetyLimits>,
) -> PyResult<ValidationReport> {
    let data = python_pdf_bytes(&data, py)?;
    let options = validation_options(py, profile, limits)?;
    py.detach(|| page_validation::validate_pdf_bytes(data.as_ref(), &options))
        .map(Into::into)
        .map_err(python_validation_error)
}

#[pymodule]
fn _page(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("PageError", py.get_type::<PageError>())?;
    module.add("ParseError", py.get_type::<ParseError>())?;
    module.add("SafetyLimitError", py.get_type::<SafetyLimitError>())?;
    module.add("ProfileError", py.get_type::<ProfileError>())?;
    module.add_class::<SafetyLimits>()?;
    module.add_class::<PdfObjectId>()?;
    module.add_class::<ValidationFailure>()?;
    module.add_class::<ValidationCounts>()?;
    module.add_class::<ValidationCheckCounts>()?;
    module.add_class::<PdfDocument>()?;
    module.add_class::<ValidationReport>()?;
    module.add_function(wrap_pyfunction!(is_pdf_compliant, module)?)?;
    module.add_function(wrap_pyfunction!(is_pdf_compliant_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(validate_pdf, module)?)?;
    module.add_function(wrap_pyfunction!(validate_pdf_bytes, module)?)?;
    Ok(())
}
