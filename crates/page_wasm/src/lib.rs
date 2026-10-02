use page_validation::{
    SafetyLimits, ValidationOptions, ValidationProfile, ValidationReport, is_pdf_compliant_bytes,
    validate_pdf_bytes,
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SafetyLimit<T> {
    Bounded(T),
    Unlimited(Unlimited),
}

#[derive(Debug, Deserialize)]
enum Unlimited {
    #[serde(rename = "unlimited")]
    Value,
}

impl<T> SafetyLimit<T> {
    fn resolve(self, unlimited: T) -> T {
        match self {
            Self::Bounded(value) => value,
            Self::Unlimited(_) => unlimited,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SafetyLimitsInput {
    max_input_size: Option<SafetyLimit<u64>>,
    max_decoded_stream_size: Option<SafetyLimit<usize>>,
    max_total_decoded_content_size: Option<SafetyLimit<usize>>,
    max_form_invocations: Option<SafetyLimit<usize>>,
    max_object_count: Option<SafetyLimit<usize>>,
    max_reference_depth: Option<SafetyLimit<usize>>,
    max_xref_revisions: Option<SafetyLimit<usize>>,
    max_table_span: Option<SafetyLimit<usize>>,
    max_table_grid_rows: Option<SafetyLimit<usize>>,
    max_table_grid_columns: Option<SafetyLimit<usize>>,
    max_table_grid_cells: Option<SafetyLimit<usize>>,
    max_unicode_cmap_mappings: Option<SafetyLimit<usize>>,
}

impl SafetyLimitsInput {
    fn into_limits(self) -> SafetyLimits {
        let mut limits = SafetyLimits::default();
        let unlimited = SafetyLimits::unlimited();
        limits.max_input_size = self.max_input_size.map_or(limits.max_input_size, |limit| {
            limit.resolve(unlimited.max_input_size)
        });
        limits.max_decoded_stream_size = self
            .max_decoded_stream_size
            .map_or(limits.max_decoded_stream_size, |limit| {
                limit.resolve(unlimited.max_decoded_stream_size)
            });
        limits.max_total_decoded_content_size = self
            .max_total_decoded_content_size
            .map_or(limits.max_total_decoded_content_size, |limit| {
                limit.resolve(unlimited.max_total_decoded_content_size)
            });
        limits.max_form_invocations = self
            .max_form_invocations
            .map_or(limits.max_form_invocations, |limit| {
                limit.resolve(unlimited.max_form_invocations)
            });
        limits.max_object_count = self
            .max_object_count
            .map_or(limits.max_object_count, |limit| {
                limit.resolve(unlimited.max_object_count)
            });
        limits.max_reference_depth = self
            .max_reference_depth
            .map_or(limits.max_reference_depth, |limit| {
                limit.resolve(unlimited.max_reference_depth)
            });
        limits.max_xref_revisions = self
            .max_xref_revisions
            .map_or(limits.max_xref_revisions, |limit| {
                limit.resolve(unlimited.max_xref_revisions)
            });
        limits.max_table_span = self.max_table_span.map_or(limits.max_table_span, |limit| {
            limit.resolve(unlimited.max_table_span)
        });
        limits.max_table_grid_rows = self
            .max_table_grid_rows
            .map_or(limits.max_table_grid_rows, |limit| {
                limit.resolve(unlimited.max_table_grid_rows)
            });
        limits.max_table_grid_columns = self
            .max_table_grid_columns
            .map_or(limits.max_table_grid_columns, |limit| {
                limit.resolve(unlimited.max_table_grid_columns)
            });
        limits.max_table_grid_cells = self
            .max_table_grid_cells
            .map_or(limits.max_table_grid_cells, |limit| {
                limit.resolve(unlimited.max_table_grid_cells)
            });
        limits.max_unicode_cmap_mappings = self
            .max_unicode_cmap_mappings
            .map_or(limits.max_unicode_cmap_mappings, |limit| {
                limit.resolve(unlimited.max_unicode_cmap_mappings)
            });
        limits
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

fn validation_options(
    profile: Option<String>,
    limits_json: Option<String>,
) -> Result<ValidationOptions, JsValue> {
    Ok(ValidationOptions::default()
        .profile(parse_profile(profile)?)
        .limits(parse_limits(limits_json)?))
}

fn report_json(report: ValidationReport) -> Result<String, JsValue> {
    serde_json::to_string(&report.json_report()).map_err(|error| {
        js_error(
            "ValidationError",
            format!("could not serialize report: {error}"),
        )
    })
}

/// Validates PDF bytes and returns the stable JSON report.
#[wasm_bindgen(js_name = validatePdfBytes)]
pub fn validate_pdf_bytes_wasm(
    bytes: &[u8],
    profile: Option<String>,
    limits_json: Option<String>,
) -> Result<String, JsValue> {
    validate_pdf_bytes(bytes, &validation_options(profile, limits_json)?)
        .map_err(|error| js_error("ValidationError", error.to_string()))
        .and_then(report_json)
}

/// Performs lazy PDF byte validation and returns only the compliance result.
#[wasm_bindgen(js_name = isPdfCompliantBytes)]
pub fn is_pdf_compliant_bytes_wasm(
    bytes: &[u8],
    profile: Option<String>,
    limits_json: Option<String>,
) -> Result<bool, JsValue> {
    is_pdf_compliant_bytes(bytes, &validation_options(profile, limits_json)?)
        .map_err(|error| js_error("ValidationError", error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{SafetyLimitsInput, parse_limits, parse_profile};
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
            r#"{"max_input_size":42,"max_reference_depth":7,"max_form_invocations":44,"max_table_span":9,"max_table_grid_rows":10,"max_table_grid_columns":11,"max_table_grid_cells":12,"max_unicode_cmap_mappings":13}"#.to_owned(),
        ))
        .expect("limits");

        assert_eq!(limits.max_input_size, 42);
        assert_eq!(limits.max_reference_depth, 7);
        assert_eq!(limits.max_form_invocations, 44);
        assert_eq!(limits.max_table_span, 9);
        assert_eq!(limits.max_table_grid_rows, 10);
        assert_eq!(limits.max_table_grid_columns, 11);
        assert_eq!(limits.max_table_grid_cells, 12);
        assert_eq!(limits.max_unicode_cmap_mappings, 13);
        assert_eq!(
            limits.max_object_count,
            SafetyLimits::DEFAULT_MAX_OBJECT_COUNT
        );
    }

    #[test]
    fn resolves_unlimited_tokens_and_preserves_finite_and_default_bounds() {
        let limits = parse_limits(Some(
            r#"{"max_input_size":"unlimited","max_decoded_stream_size":"unlimited","max_reference_depth":7}"#.to_owned(),
        ))
        .expect("unlimited and mixed limits");
        assert_eq!(limits.max_input_size, u64::MAX);
        assert_eq!(limits.max_decoded_stream_size, usize::MAX);
        assert_eq!(limits.max_reference_depth, 7);
        assert_eq!(
            limits.max_object_count,
            SafetyLimits::DEFAULT_MAX_OBJECT_COUNT
        );
    }

    #[test]
    fn rejects_invalid_limit_tokens_and_numbers() {
        for value in [r#""Infinity""#, r#""disabled""#, "-1", "0.5"] {
            let json = format!(r#"{{"max_input_size":{value}}}"#);
            serde_json::from_str::<SafetyLimitsInput>(&json)
                .expect_err("invalid safety-limit input");
        }
    }
}
