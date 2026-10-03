from enum import Enum


class ValidationProfile(Enum):
    """A PDF/A or PDF/UA profile available in this version of page."""

    PDF_A_1B = ("1b", True)
    PDF_A_1A = ("1a", True)
    PDF_A_2B = ("2b", True)
    PDF_A_2A = ("2a", True)
    PDF_A_2U = ("2u", True)
    PDF_A_3B = ("3b", True)
    PDF_A_3A = ("3a", True)
    PDF_A_3U = ("3u", True)
    PDF_UA_1 = ("ua1", True)

    _implemented: bool

    def __new__(cls, value: str, is_implemented: bool):
        member = object.__new__(cls)
        member._value_ = value
        member._implemented = is_implemented
        return member

    @property
    def is_implemented(self) -> bool:
        """Whether page implements validation for this profile."""
        return self._implemented


class FailureCategory(Enum):
    """The type of a validation finding recorded in a report."""

    METADATA = "metadata"
    CONFORMANCE = "conformance"
