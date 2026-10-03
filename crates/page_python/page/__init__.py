from importlib.metadata import version

from page._page import (
    FailureCategory,
    PdfDocument,
    PdfObjectId,
    ParseError,
    ProfileError,
    SafetyLimits,
    SafetyLimitError,
    ValidationCheckCounts,
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

__version__ = version("page-validation")

__all__ = [
    "FailureCategory",
    "PdfDocument",
    "PdfObjectId",
    "ParseError",
    "ProfileError",
    "SafetyLimits",
    "SafetyLimitError",
    "ValidationCheckCounts",
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
