from importlib.metadata import version
from pathlib import Path
from struct import calcsize

import page
import pytest


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
    assert page.__version__ == version("page-validation")


def test_default_safety_limits():
    limits = page.SafetyLimits()

    assert limits.max_input_size == page.SafetyLimits.DEFAULT_MAX_INPUT_SIZE
    assert limits.max_decoded_stream_size == 32 * 1024 * 1024
    assert (
        limits.max_total_decoded_content_size
        == page.SafetyLimits.DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE
    )
    assert limits.max_form_invocations == page.SafetyLimits.DEFAULT_MAX_FORM_INVOCATIONS
    assert limits.max_object_count == 1_000_000
    assert limits.max_reference_depth == 256
    assert limits.max_xref_revisions == page.SafetyLimits.DEFAULT_MAX_XREF_REVISIONS
    assert limits.max_table_span == page.SafetyLimits.DEFAULT_MAX_TABLE_SPAN
    assert limits.max_table_grid_rows == page.SafetyLimits.DEFAULT_MAX_TABLE_GRID_ROWS
    assert (
        limits.max_table_grid_columns
        == page.SafetyLimits.DEFAULT_MAX_TABLE_GRID_COLUMNS
    )
    assert limits.max_table_grid_cells == page.SafetyLimits.DEFAULT_MAX_TABLE_GRID_CELLS
    assert (
        limits.max_unicode_cmap_mappings
        == page.SafetyLimits.DEFAULT_MAX_UNICODE_CMAP_MAPPINGS
    )


def test_custom_safety_limits():
    limits = page.SafetyLimits(
        max_input_size=42,
        max_total_decoded_content_size=43,
        max_form_invocations=44,
        max_reference_depth=7,
        max_xref_revisions=8,
        max_table_span=9,
        max_table_grid_rows=10,
        max_table_grid_columns=11,
        max_table_grid_cells=12,
        max_unicode_cmap_mappings=13,
    )
    assert limits.max_input_size == 42
    assert limits.max_total_decoded_content_size == 43
    assert limits.max_form_invocations == 44
    assert limits.max_reference_depth == 7
    assert limits.max_xref_revisions == 8
    assert limits.max_table_span == 9
    assert limits.max_table_grid_rows == 10
    assert limits.max_table_grid_columns == 11
    assert limits.max_table_grid_cells == 12
    assert limits.max_unicode_cmap_mappings == 13


def test_unlimited_safety_limits():
    limits = page.SafetyLimits.unlimited()
    assert limits.max_input_size == 2**64 - 1
    native_max = 2 ** (8 * calcsize("P")) - 1
    for name in (
        "max_decoded_stream_size",
        "max_total_decoded_content_size",
        "max_form_invocations",
        "max_object_count",
        "max_reference_depth",
        "max_xref_revisions",
        "max_table_span",
        "max_table_grid_rows",
        "max_table_grid_columns",
        "max_table_grid_cells",
        "max_unicode_cmap_mappings",
    ):
        assert getattr(limits, name) == native_max


def test_unlimited_limits_allow_restoring_an_independent_bound():
    limits = page.SafetyLimits.unlimited()
    limits.max_input_size = 1
    assert page.SafetyLimits.unlimited().max_input_size == 2**64 - 1

    with pytest.raises(page.ValidationError, match="1-byte limit"):
        page.validate_pdf_bytes(
            minimal_pdf(), profile=page.ValidationProfile.PDF_A_1B, limits=limits
        )


def test_unlimited_limits_work_with_file_and_byte_apis(tmp_path: Path):
    data = minimal_pdf()
    path = tmp_path / "trusted.pdf"
    path.write_bytes(data)
    limits = page.SafetyLimits.unlimited()
    profile = page.ValidationProfile.PDF_A_1B

    assert page.validate_pdf(path, profile=profile, limits=limits).is_compliant is False
    assert (
        page.validate_pdf_bytes(data, profile=profile, limits=limits).is_compliant
        is False
    )
    assert page.is_pdf_compliant(path, profile=profile, limits=limits) is False
    assert page.is_pdf_compliant_bytes(data, profile=profile, limits=limits) is False


def test_compliance_api_rejects_invalid_bytes():
    with pytest.raises(page.ParseError):
        page.is_pdf_compliant_bytes(
            b"not a PDF", profile=page.ValidationProfile.PDF_A_1B
        )


def test_validation_api_raises_for_invalid_bytes():
    with pytest.raises(page.ParseError):
        page.validate_pdf_bytes(b"not a PDF", profile=page.ValidationProfile.PDF_A_1B)


def test_validation_and_compliance_apis_return_expected_values():
    data = minimal_pdf()

    compliance = page.is_pdf_compliant_bytes(
        data, profile=page.ValidationProfile.PDF_A_1B
    )
    report = page.validate_pdf_bytes(data, profile=page.ValidationProfile.PDF_A_1B)

    assert compliance is False
    assert report.profile == page.ValidationProfile.PDF_A_1B
    assert report.rules.total > 0
    assert report.rules.failed > 0
    assert report.checks.failed > 0
    assert report.is_compliant is False
    assert not hasattr(report, "exit_code")
    assert report.failures


def test_compliance_file_api_returns_bool(tmp_path: Path):
    pdf_path = tmp_path / "document.pdf"
    pdf_path.write_bytes(minimal_pdf())

    assert (
        page.is_pdf_compliant(pdf_path, profile=page.ValidationProfile.PDF_A_1B)
        is False
    )


def test_validation_reports_missing_file_as_file_not_found(tmp_path: Path):
    missing_file = tmp_path / "missing.pdf"
    with pytest.raises(FileNotFoundError):
        page.validate_pdf(missing_file, profile=page.ValidationProfile.PDF_A_1B)


def test_validation_reports_other_file_read_failures_as_os_error(tmp_path: Path):
    with pytest.raises(OSError):
        page.validate_pdf(tmp_path, profile=page.ValidationProfile.PDF_A_1B)


def test_validation_errors_have_specific_base_classes():
    assert issubclass(page.ParseError, page.ValidationError)
    assert issubclass(page.SafetyLimitError, page.ValidationError)
    assert issubclass(page.ProfileError, page.ValidationError)

    with pytest.raises(page.SafetyLimitError):
        page.validate_pdf_bytes(
            minimal_pdf(),
            profile=page.ValidationProfile.PDF_A_1B,
            limits=page.SafetyLimits(max_input_size=1),
        )

    with pytest.raises(page.ProfileError):
        page.validate_pdf_bytes(minimal_pdf())


def test_validation_options_are_keyword_only():
    data = minimal_pdf()
    profile = page.ValidationProfile.PDF_A_1B

    with pytest.raises(TypeError):
        page.validate_pdf_bytes(data, profile)  # type: ignore
    with pytest.raises(TypeError):
        page.is_pdf_compliant_bytes(data, profile)  # type: ignore


def test_compliance_api_exposes_boolean_results():
    assert isinstance(
        page.is_pdf_compliant_bytes(
            minimal_pdf(), profile=page.ValidationProfile.PDF_A_1B
        ),
        bool,
    )
    assert hasattr(page, "is_pdf_compliant")
    assert hasattr(page, "is_pdf_compliant_bytes")
    assert hasattr(page, "validate_pdf")
    assert hasattr(page, "validate_pdf_bytes")
