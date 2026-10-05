from enum import Enum


class ValidationProfile(Enum):
    """A PDF/A or PDF/UA profile available in this version of page."""

    PDF_A_1B = "1b"
    PDF_A_1A = "1a"
    PDF_A_2B = "2b"
    PDF_A_2A = "2a"
    PDF_A_2U = "2u"
    PDF_A_3B = "3b"
    PDF_A_3A = "3a"
    PDF_A_3U = "3u"
    PDF_UA_1 = "ua1"


class FailureCategory(Enum):
    """The type of a validation finding recorded in a report."""

    METADATA = "metadata"
    CONFORMANCE = "conformance"
