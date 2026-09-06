from pathlib import Path

import pytest

import page


def minimal_pdf() -> bytes:
    objects = [
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        b"2 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n",
    ]
    data = b"%PDF-1.4\n"
    offsets = []
    for pdf_object in objects:
        offsets.append(len(data))
        data += pdf_object

    xref_offset = len(data)
    data += b"xref\n0 3\n0000000000 65535 f \n"
    data += b"".join(f"{offset:010d} 00000 n \n".encode() for offset in offsets)
    data += (
        b"trailer\n<< /Size 3 /Root 1 0 R >>\nstartxref\n"
        + str(xref_offset).encode()
        + b"\n%%EOF\n"
    )
    return data


def test_version():
    assert page.__version__ == "0.6.0"


def test_default_safety_limits():
    limits = page.SafetyLimits()

    assert limits.max_input_size == page.SafetyLimits.DEFAULT_MAX_INPUT_SIZE
    assert limits.max_decoded_stream_size == 32 * 1024 * 1024
    assert (
        limits.max_total_decoded_content_size
        == page.SafetyLimits.DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE
    )
    assert limits.max_object_count == 1_000_000
    assert limits.max_reference_depth == 256
    assert limits.max_xref_revisions == page.SafetyLimits.DEFAULT_MAX_XREF_REVISIONS


def test_custom_safety_limits():
    limits = page.SafetyLimits(
        max_input_size=42,
        max_total_decoded_content_size=43,
        max_reference_depth=7,
        max_xref_revisions=8,
    )

    assert limits.max_input_size == 42
    assert limits.max_total_decoded_content_size == 43
    assert limits.max_reference_depth == 7
    assert limits.max_xref_revisions == 8


def test_compliance_api_rejects_invalid_bytes():
    with pytest.raises(page.ValidationError):
        page.is_pdf_compliant_bytes(b"not a PDF", page.ValidationProfile.PDF_A_1B)


def test_validation_api_raises_for_invalid_bytes():
    with pytest.raises(page.ValidationError):
        page.validate_pdf_bytes(b"not a PDF", page.ValidationProfile.PDF_A_1B)


def test_validation_and_compliance_apis_return_expected_values():
    data = minimal_pdf()

    compliance = page.is_pdf_compliant_bytes(data, page.ValidationProfile.PDF_A_1B)
    report = page.validate_pdf_bytes(data, page.ValidationProfile.PDF_A_1B)

    assert compliance is False
    assert report.profile == page.ValidationProfile.PDF_A_1B
    assert report.is_compliant is False
    assert report.failures


def test_compliance_file_api_returns_bool(tmp_path: Path):
    pdf_path = tmp_path / "document.pdf"
    pdf_path.write_bytes(minimal_pdf())

    assert page.is_pdf_compliant(pdf_path, page.ValidationProfile.PDF_A_1B) is False


def test_validation_reports_missing_file_as_error(tmp_path: Path):
    missing_file = tmp_path / "missing.pdf"
    with pytest.raises(page.ValidationError, match="could not read input"):
        page.validate_pdf(missing_file, page.ValidationProfile.PDF_A_1B)


def test_compliance_api_exposes_boolean_results():
    assert isinstance(
        page.is_pdf_compliant_bytes(minimal_pdf(), page.ValidationProfile.PDF_A_1B),
        bool,
    )
    assert hasattr(page, "is_pdf_compliant")
    assert hasattr(page, "is_pdf_compliant_bytes")
    assert hasattr(page, "validate_pdf")
    assert hasattr(page, "validate_pdf_bytes")
