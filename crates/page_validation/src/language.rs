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
    inspect_dictionary_with(
        document,
        limits,
        dictionary,
        object_id,
        context,
        is_language_tag,
    )
}

pub(crate) fn inspect_dictionary_pdfa23(
    document: &Document,
    limits: &SafetyLimits,
    dictionary: &Dictionary,
    object_id: Option<PdfObjectId>,
    context: &str,
) -> Option<RuleFailure> {
    inspect_dictionary_with(
        document,
        limits,
        dictionary,
        object_id,
        context,
        is_language_tag_pdfa23,
    )
}

pub(crate) fn inspect_dictionary_pdfua1(
    document: &Document,
    limits: &SafetyLimits,
    dictionary: &Dictionary,
    object_id: Option<PdfObjectId>,
    context: &str,
) -> Option<RuleFailure> {
    inspect_dictionary_with(
        document,
        limits,
        dictionary,
        object_id,
        context,
        is_language_tag_pdfua1,
    )
}

fn inspect_dictionary_with(
    document: &Document,
    limits: &SafetyLimits,
    dictionary: &Dictionary,
    object_id: Option<PdfObjectId>,
    context: &str,
    predicate: fn(&str) -> bool,
) -> Option<RuleFailure> {
    let value = dictionary.get(b"Lang").ok()?;
    let value =
        crate::object_resolution::resolve_optional(document, value, limits.max_reference_depth)
            .ok()
            .flatten();
    let valid = match value {
        Some(Object::String(bytes, _)) => {
            predicate(&crate::model::decode_verapdf_pdf_string(bytes))
        }
        // veraPDF creates a CosLang rule object only for string-valued Lang
        // entries; nulls and other COS types are not evaluated by the
        // language-tag profiles.
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

pub(crate) fn is_language_tag_pdfa23(value: &str) -> bool {
    value.is_empty()
        || value.split('-').enumerate().all(|(index, component)| {
            (1..=8).contains(&component.len())
                && component.is_ascii()
                && component.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() && (index > 0 || byte.is_ascii_alphabetic())
                })
        })
}

pub(crate) fn is_language_tag_pdfua1(value: &str) -> bool {
    let mut components = value.split('-');
    components.next().is_some_and(|primary| {
        (1..=8).contains(&primary.len())
            && primary.is_ascii()
            && primary.bytes().all(|byte| byte.is_ascii_alphabetic())
            && components.all(|component| {
                (1..=8).contains(&component.len())
                    && component.is_ascii()
                    && component.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
    })
}

#[cfg(test)]
mod tests {
    use super::{is_language_tag, is_language_tag_pdfa23, is_language_tag_pdfua1};

    #[test]
    fn matches_verapdf_language_tag_predicate() {
        for value in ["", "en", "EN", "en-US", "i-klingon"] {
            assert!(is_language_tag(value), "{value}");
        }
        for value in ["en-", "-en", "en--US", "englishhh", "en_US", "en-123", "é"] {
            assert!(!is_language_tag(value), "{value}");
        }
    }

    #[test]
    fn pdfa23_language_tags_allow_alphanumeric_subtags() {
        assert!(is_language_tag_pdfa23("ru-petr1708"));
        assert!(is_language_tag_pdfa23("en-US-123"));
        assert!(!is_language_tag_pdfa23("12-BE"));
        assert!(!is_language_tag_pdfa23("en/US"));
    }

    #[test]
    fn pdfua1_language_tags_match_the_profile_predicate() {
        for value in ["en", "EN", "en-US", "i-klingon", "en-123"] {
            assert!(is_language_tag_pdfua1(value), "{value}");
        }
        for value in [
            "",
            "en-",
            "-en",
            "en--US",
            "englishhh",
            "en_US",
            "12-en",
            "é",
        ] {
            assert!(!is_language_tag_pdfua1(value), "{value}");
        }
    }
}
