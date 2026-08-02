//! WebAssembly bindings for validating PDF bytes with `page_validation`.

use js_sys::Error;
use page_validation::{
    SafetyLimits, ValidationProfile, ValidationReport, validate_bytes, validate_bytes_with_profile,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_TYPES: &str = r#"
export type ValidationProfile =
    | "a-1b"
    | "a-1a"
    | "a-2b"
    | "a-2a"
    | "a-2u"
    | "a-3b"
    | "a-3a"
    | "a-3u"
    | "a-4"
    | "a-4e"
    | "a-4f"
    | "ua-1"
    | "ua-2";

export interface ValidationFailure {
    rule: string;
    message: string;
}

export interface ValidationError {
    kind: "parser" | "operational";
    rule: string;
    message: string;
}

export interface ValidationReport {
    profile: ValidationProfile;
    valid: boolean;
    failures: ValidationFailure[];
    error?: ValidationError;
}
"#;

/// Validates PDF bytes against the PDF/A profile declared in their XMP metadata.
///
/// The JavaScript API accepts a `Uint8Array` and returns the stable validation
/// report used by the other `page` bindings. It throws a JavaScript `Error` if
/// the input cannot be parsed or its profile declaration is missing, malformed,
/// unsupported, or exceeds a safety limit.
#[wasm_bindgen(
    js_name = validatePdf,
    unchecked_return_type = "ValidationReport"
)]
pub fn validate_pdf(data: &[u8]) -> Result<JsValue, JsValue> {
    let report = validate_bytes(data, &SafetyLimits::default())
        .map_err(|error| Error::new(&error.to_string()))?;

    report_to_js(&report)
}

/// Validates PDF bytes against an explicitly selected PDF/A or PDF/UA profile.
///
/// Unlike [`validate_pdf`], this function does not require a profile declaration
/// in the document. Parser, operational, and conformance failures are returned
/// in the report. An unknown profile string throws a JavaScript `Error`.
#[wasm_bindgen(
    js_name = validatePdfWithProfile,
    unchecked_return_type = "ValidationReport"
)]
pub fn validate_pdf_with_profile(
    data: &[u8],
    #[wasm_bindgen(unchecked_param_type = "ValidationProfile")] profile: &str,
) -> Result<JsValue, JsValue> {
    let profile = parse_profile(profile)
        .ok_or_else(|| Error::new(&format!("unknown validation profile: {profile}")))?;
    let report = validate_bytes_with_profile(data, profile, &SafetyLimits::default());

    report_to_js(&report)
}

fn report_to_js(report: &ValidationReport) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&report.json_report())
        .map_err(|error| Error::new(&error.to_string()).into())
}

fn parse_profile(profile: &str) -> Option<ValidationProfile> {
    match profile {
        "a-1b" => Some(ValidationProfile::PdfA1b),
        "a-1a" => Some(ValidationProfile::PdfA1a),
        "a-2b" => Some(ValidationProfile::PdfA2b),
        "a-2a" => Some(ValidationProfile::PdfA2a),
        "a-2u" => Some(ValidationProfile::PdfA2u),
        "a-3b" => Some(ValidationProfile::PdfA3b),
        "a-3a" => Some(ValidationProfile::PdfA3a),
        "a-3u" => Some(ValidationProfile::PdfA3u),
        "a-4" => Some(ValidationProfile::PdfA4),
        "a-4e" => Some(ValidationProfile::PdfA4e),
        "a-4f" => Some(ValidationProfile::PdfA4f),
        "ua-1" => Some(ValidationProfile::PdfUa1),
        "ua-2" => Some(ValidationProfile::PdfUa2),
        _ => None,
    }
}

#[cfg(all(test, target_arch = "wasm32", target_os = "unknown"))]
mod tests {
    use js_sys::{Array, Error, Reflect};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::{validate_pdf, validate_pdf_with_profile};

    const NONCOMPLIANT_PDF: &[u8] =
        include_bytes!("../../page_validation/tests/fixtures/trailer-id-missing.pdf");
    const PDF_WITHOUT_PROFILE: &[u8] =
        include_bytes!("../../page_validation/tests/fixtures/structural.pdf");

    fn property(value: &JsValue, name: &str) -> JsValue {
        Reflect::get(value, &JsValue::from_str(name)).expect("read report property")
    }

    #[wasm_bindgen_test]
    fn returns_the_stable_validation_report() {
        let report = validate_pdf(NONCOMPLIANT_PDF).expect("validate PDF");

        assert_eq!(
            property(&report, "profile").as_string().as_deref(),
            Some("a-1b")
        );
        assert_eq!(property(&report, "valid").as_bool(), Some(false));
        assert!(!Reflect::has(&report, &JsValue::from_str("file")).expect("inspect report"));

        let failures = Array::from(&property(&report, "failures"));
        assert_eq!(failures.length(), 1);
        assert_eq!(
            property(&failures.get(0), "rule").as_string().as_deref(),
            Some("PDFA1B-TRAILER-ID-001")
        );
    }

    #[wasm_bindgen_test]
    fn throws_an_error_for_invalid_input() {
        let error = validate_pdf(b"not a PDF").expect_err("reject invalid PDF");
        let error = error.dyn_into::<Error>().expect("JavaScript Error");

        assert!(
            error
                .message()
                .as_string()
                .expect("error message")
                .contains("PDF parser rejected the input")
        );
    }

    #[wasm_bindgen_test]
    fn validates_with_an_explicit_profile() {
        let report = validate_pdf_with_profile(PDF_WITHOUT_PROFILE, "a-1b")
            .expect("validate with explicit profile");

        assert_eq!(
            property(&report, "profile").as_string().as_deref(),
            Some("a-1b")
        );
        assert_eq!(property(&report, "valid").as_bool(), Some(false));
        assert!(property(&report, "error").is_undefined());
    }

    #[wasm_bindgen_test]
    fn rejects_an_unknown_explicit_profile() {
        let error = validate_pdf_with_profile(PDF_WITHOUT_PROFILE, "pdf-a-1b")
            .expect_err("reject unknown profile");
        let error = error.dyn_into::<Error>().expect("JavaScript Error");

        assert_eq!(
            error.message().as_string().as_deref(),
            Some("unknown validation profile: pdf-a-1b")
        );
    }

    #[wasm_bindgen_test]
    fn accepts_every_documented_profile_name() {
        for profile in [
            "a-1b", "a-1a", "a-2b", "a-2a", "a-2u", "a-3b", "a-3a", "a-3u", "a-4", "a-4e", "a-4f",
            "ua-1", "ua-2",
        ] {
            validate_pdf_with_profile(b"not a PDF", profile)
                .unwrap_or_else(|_| panic!("accept profile {profile}"));
        }
    }
}
