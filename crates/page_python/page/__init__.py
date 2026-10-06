from importlib.metadata import version

from page._enums import FailureCategory, ValidationProfile
from page._page import (
    PageError,
    ParseError,
    PdfDocument,
    PdfObjectId,
    ProfileError,
    SafetyLimitError,
    SafetyLimits,
    ValidationCheckCounts,
    ValidationCounts,
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
    "PageError",
    "ParseError",
    "PdfDocument",
    "PdfObjectId",
    "ProfileError",
    "SafetyLimitError",
    "SafetyLimits",
    "ValidationCheckCounts",
    "ValidationCounts",
    "ValidationFailure",
    "ValidationProfile",
    "ValidationReport",
    "is_pdf_compliant",
    "is_pdf_compliant_bytes",
    "validate_pdf",
    "validate_pdf_bytes",
]
