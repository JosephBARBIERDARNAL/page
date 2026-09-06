from os import PathLike
from typing import ClassVar

class ValidationError(Exception): ...

class ValidationProfile:
    PDF_A_1B: ClassVar[ValidationProfile]
    PDF_A_1A: ClassVar[ValidationProfile]
    PDF_A_2B: ClassVar[ValidationProfile]
    PDF_A_2A: ClassVar[ValidationProfile]
    PDF_A_2U: ClassVar[ValidationProfile]
    PDF_A_3B: ClassVar[ValidationProfile]
    PDF_A_3A: ClassVar[ValidationProfile]
    PDF_A_3U: ClassVar[ValidationProfile]
    PDF_A_4: ClassVar[ValidationProfile]
    PDF_A_4E: ClassVar[ValidationProfile]
    PDF_A_4F: ClassVar[ValidationProfile]
    PDF_UA_1: ClassVar[ValidationProfile]
    PDF_UA_2: ClassVar[ValidationProfile]

class FailureCategory:
    OPERATIONAL: ClassVar[FailureCategory]
    PARSER: ClassVar[FailureCategory]
    METADATA: ClassVar[FailureCategory]
    CONFORMANCE: ClassVar[FailureCategory]

class SafetyLimits:
    DEFAULT_MAX_INPUT_SIZE: ClassVar[int]
    DEFAULT_MAX_DECODED_STREAM_SIZE: ClassVar[int]
    DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE: ClassVar[int]
    DEFAULT_MAX_OBJECT_COUNT: ClassVar[int]
    DEFAULT_MAX_REFERENCE_DEPTH: ClassVar[int]
    DEFAULT_MAX_XREF_REVISIONS: ClassVar[int]

    max_input_size: int
    max_decoded_stream_size: int
    max_total_decoded_content_size: int
    max_object_count: int
    max_reference_depth: int
    max_xref_revisions: int

    def __init__(
        self,
        *,
        max_input_size: int | None = None,
        max_decoded_stream_size: int | None = None,
        max_total_decoded_content_size: int | None = None,
        max_object_count: int | None = None,
        max_reference_depth: int | None = None,
        max_xref_revisions: int | None = None,
    ) -> None: ...

class PdfObjectId:
    @property
    def object_number(self) -> int: ...
    @property
    def generation(self) -> int: ...

class ValidationFailure:
    @property
    def rule_id(self) -> str: ...
    @property
    def message(self) -> str: ...
    @property
    def object_id(self) -> PdfObjectId | None: ...
    @property
    def category(self) -> FailureCategory: ...

class ValidationCounts:
    @property
    def total(self) -> int: ...
    @property
    def passed(self) -> int: ...
    @property
    def failed(self) -> int: ...

class PdfDocument:
    @property
    def version(self) -> str: ...
    @property
    def encrypted(self) -> bool: ...
    @property
    def page_count(self) -> int: ...
    @property
    def object_count(self) -> int: ...

class ValidationReport:
    @property
    def profile(self) -> ValidationProfile: ...
    @property
    def is_compliant(self) -> bool: ...
    @property
    def preliminary(self) -> bool: ...
    @property
    def checks(self) -> ValidationCounts: ...
    @property
    def document(self) -> PdfDocument | None: ...
    @property
    def failures(self) -> list[ValidationFailure]: ...
    def has_operational_failure(self) -> bool: ...
    def exit_code(self) -> int: ...
    def to_json(self) -> str: ...

def is_pdf_compliant(
    path: str | PathLike[str],
    profile: ValidationProfile | None = None,
    limits: SafetyLimits | None = None,
) -> bool: ...
def is_pdf_compliant_bytes(
    data: bytes,
    profile: ValidationProfile | None = None,
    limits: SafetyLimits | None = None,
) -> bool: ...
def validate_pdf(
    path: str | PathLike[str],
    profile: ValidationProfile | None = None,
    limits: SafetyLimits | None = None,
) -> ValidationReport: ...
def validate_pdf_bytes(
    data: bytes,
    profile: ValidationProfile | None = None,
    limits: SafetyLimits | None = None,
) -> ValidationReport: ...
