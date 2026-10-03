from importlib.metadata import version

from page._enums import FailureCategory, ValidationProfile
from page._page import (
    ParseError,
    PdfDocument,
    PdfObjectId,
    ProfileError,
    SafetyLimitError,
    SafetyLimits,
    ValidationCheckCounts,
    ValidationCounts,
    ValidationError,
    ValidationFailure,
    ValidationReport,
    is_pdf_compliant,
    is_pdf_compliant_bytes,
    validate_pdf,
    validate_pdf_bytes,
)

__version__ = version("page-validation")

__all__ = [
    "FailureCategory",
    "ParseError",
    "PdfDocument",
    "PdfObjectId",
    "ProfileError",
    "SafetyLimitError",
    "SafetyLimits",
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
