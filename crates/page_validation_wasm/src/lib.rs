//! WebAssembly bindings for validating PDF bytes with `page_validation`.

use js_sys::Error;
use page_validation::{SafetyLimits, validate_bytes};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_TYPES: &str = r#"
export type ValidationProfile = "a-1b";

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

    serde_wasm_bindgen::to_value(&report.json_report())
        .map_err(|error| Error::new(&error.to_string()).into())
}

#[cfg(all(test, target_arch = "wasm32", target_os = "unknown"))]
mod tests {
    use js_sys::{Array, Error, Reflect};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::validate_pdf;

    const NONCOMPLIANT_PDF: &[u8] =
        include_bytes!("../../page_validation/tests/fixtures/trailer-id-missing.pdf");

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
}
