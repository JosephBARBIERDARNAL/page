from importlib.metadata import version as _version

from page._page import (
    FailureCategory,
    PdfDocument,
    PdfObjectId,
    SafetyLimits,
    ValidationCounts,
    ValidationError,
    ValidationFailure,
    ValidationProfile,
    ValidationReport,
    is_pdf_compliant,
    is_pdf_compliant_bytes,
    validate_pdf,
    validate_pdf_bytes,
)

__version__ = _version("page-validation")

__all__ = [
    "FailureCategory",
    "PdfDocument",
    "PdfObjectId",
    "SafetyLimits",
    "ValidationCounts",
    "ValidationError",
    "ValidationFailure",
    "ValidationProfile",
    "ValidationReport",
    "is_pdf_compliant",
    "is_pdf_compliant_bytes",
    "validate_pdf",
    "validate_pdf_bytes",
]
