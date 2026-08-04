use lopdf::{Dictionary, Document, Object};

use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::report::RuleFailure;

pub(crate) fn inspect_dictionary(
    document: &Document,
    limits: &SafetyLimits,
    dictionary: &Dictionary,
    object_id: Option<PdfObjectId>,
    context: &str,
) -> Option<RuleFailure> {
    let value = dictionary.get(b"Lang").ok()?;
    let value =
        crate::object_resolution::resolve_optional(document, value, limits.max_reference_depth)
            .ok()
            .flatten();
    let valid = match value {
        Some(Object::String(bytes, _)) => {
            is_language_tag(&crate::model::decode_verapdf_pdf_string(bytes))
        }
        // veraPDF creates a CosLang rule object only for string-valued Lang
        // entries; nulls and other COS types are not evaluated by 6.8.4-1.
        _ => true,
    };
    (!valid).then(|| RuleFailure {
        object_id,
        description: format!("{context} /Lang is not an RFC 1766 language identifier"),
    })
}

pub(crate) fn is_language_tag(value: &str) -> bool {
    value.is_empty()
        || value.split('-').all(|component| {
            (1..=8).contains(&component.len())
                && component.is_ascii()
                && component.bytes().all(|byte| byte.is_ascii_alphabetic())
        })
}

#[cfg(test)]
mod tests {
    use super::is_language_tag;

    #[test]
    fn matches_verapdf_language_tag_predicate() {
        for value in ["", "en", "EN", "en-US", "i-klingon"] {
            assert!(is_language_tag(value), "{value}");
        }
        for value in ["en-", "-en", "en--US", "englishhh", "en_US", "en-123", "é"] {
            assert!(!is_language_tag(value), "{value}");
        }
    }
}
