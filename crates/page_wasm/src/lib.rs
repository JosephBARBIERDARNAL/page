use page_validation::{
    SafetyLimits, ValidationProfile, ValidationReport, is_pdf_compliant_bytes, validate_pdf_bytes,
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Deserialize)]
struct SafetyLimitsInput {
    max_input_size: Option<u64>,
    max_decoded_stream_size: Option<usize>,
    max_total_decoded_content_size: Option<usize>,
    max_object_count: Option<usize>,
    max_reference_depth: Option<usize>,
    max_xref_revisions: Option<usize>,
}

impl SafetyLimitsInput {
    fn into_limits(self) -> SafetyLimits {
        let defaults = SafetyLimits::default();
        SafetyLimits {
            max_input_size: self.max_input_size.unwrap_or(defaults.max_input_size),
            max_decoded_stream_size: self
                .max_decoded_stream_size
                .unwrap_or(defaults.max_decoded_stream_size),
            max_total_decoded_content_size: self
                .max_total_decoded_content_size
                .unwrap_or(defaults.max_total_decoded_content_size),
            max_object_count: self.max_object_count.unwrap_or(defaults.max_object_count),
            max_reference_depth: self
                .max_reference_depth
                .unwrap_or(defaults.max_reference_depth),
            max_xref_revisions: self
                .max_xref_revisions
                .unwrap_or(defaults.max_xref_revisions),
        }
    }
}

fn js_error(name: &str, message: impl AsRef<str>) -> JsValue {
    let error = js_sys::Error::new(message.as_ref());
    error.set_name(name);
    error.into()
}

fn parse_profile(profile: Option<String>) -> Result<Option<ValidationProfile>, JsValue> {
    profile
        .map(|profile| match profile.as_str() {
            "1a" => Ok(ValidationProfile::PdfA1a),
            "1b" => Ok(ValidationProfile::PdfA1b),
            "2a" => Ok(ValidationProfile::PdfA2a),
            "2b" => Ok(ValidationProfile::PdfA2b),
            "2u" => Ok(ValidationProfile::PdfA2u),
            "3a" => Ok(ValidationProfile::PdfA3a),
            "3b" => Ok(ValidationProfile::PdfA3b),
            "3u" => Ok(ValidationProfile::PdfA3u),
            "4" => Ok(ValidationProfile::PdfA4),
            "4e" => Ok(ValidationProfile::PdfA4e),
            "4f" => Ok(ValidationProfile::PdfA4f),
            "ua1" => Ok(ValidationProfile::PdfUa1),
            "ua2" => Ok(ValidationProfile::PdfUa2),
            value => Err(js_error(
                "ValidationError",
                format!("unknown validation profile: {value}"),
            )),
        })
        .transpose()
}

fn parse_limits(limits_json: Option<String>) -> Result<SafetyLimits, JsValue> {
    limits_json
        .map(|json| {
            serde_json::from_str::<SafetyLimitsInput>(&json)
                .map(SafetyLimitsInput::into_limits)
                .map_err(|error| js_error("TypeError", format!("invalid safety limits: {error}")))
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn report_json(report: ValidationReport) -> Result<String, JsValue> {
    serde_json::to_string(&report).map_err(|error| {
        js_error(
            "ValidationError",
            format!("could not serialize report: {error}"),
        )
    })
}

/// Validates PDF bytes and returns the complete structured report as JSON.
#[wasm_bindgen(js_name = validatePdfBytes)]
pub fn validate_pdf_bytes_wasm(
    bytes: &[u8],
    profile: Option<String>,
    limits_json: Option<String>,
) -> Result<String, JsValue> {
    let profile = parse_profile(profile)?;
    let limits = parse_limits(limits_json)?;
    validate_pdf_bytes(bytes, profile, &limits)
        .map_err(|error| js_error("ValidationError", error.to_string()))
        .and_then(report_json)
}

/// Performs fast PDF byte validation and returns only the compliance result.
#[wasm_bindgen(js_name = isPdfCompliantBytes)]
pub fn is_pdf_compliant_bytes_wasm(
    bytes: &[u8],
    profile: Option<String>,
    limits_json: Option<String>,
) -> Result<bool, JsValue> {
    let profile = parse_profile(profile)?;
    let limits = parse_limits(limits_json)?;
    is_pdf_compliant_bytes(bytes, profile, &limits)
        .map_err(|error| js_error("ValidationError", error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{parse_limits, parse_profile};
    use page_validation::{SafetyLimits, ValidationProfile};

    #[test]
    fn parses_all_profile_names() {
        assert_eq!(
            parse_profile(Some("1b".to_owned())).expect("profile"),
            Some(ValidationProfile::PdfA1b)
        );
        assert_eq!(
            parse_profile(Some("ua1".to_owned())).expect("profile"),
            Some(ValidationProfile::PdfUa1)
        );
        assert_eq!(parse_profile(None).expect("profile"), None);
    }

    #[test]
    fn applies_partial_safety_limits_over_defaults() {
        let limits = parse_limits(Some(
            r#"{"max_input_size":42,"max_reference_depth":7}"#.to_owned(),
        ))
        .expect("limits");

        assert_eq!(limits.max_input_size, 42);
        assert_eq!(limits.max_reference_depth, 7);
        assert_eq!(
            limits.max_object_count,
            SafetyLimits::DEFAULT_MAX_OBJECT_COUNT
        );
    }
}
