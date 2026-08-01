use lopdf::{Dictionary, Document};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::resolve_optional;

/// The document catalog together with the object identity used to attribute
/// failures to it. `object_id` is never `None`: a `Catalog` is only produced
/// once the trailer `/Root` has been confirmed indirect.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Catalog<'a> {
    pub(crate) dictionary: &'a Dictionary,
    pub(crate) object_id: PdfObjectId,
}

/// Resolves the document catalog from the trailer `/Root` entry, the single
/// entry point every catalog-graph consumer (page tree, name trees,
/// `AcroForm`, `OpenAction`, `OCProperties`, ...) must share instead of each
/// independently re-deriving it with subtly different rules.
///
/// PDF32000 §7.5.2 requires `/Root` itself to be an indirect reference, so a
/// direct dictionary embedded in the trailer is rejected even when it would
/// otherwise resolve to a well-formed `/Type /Catalog` dictionary. A resolved
/// dictionary is otherwise accepted as the catalog only when its `/Type` is
/// present and equal to `Catalog`, matching veraPDF's own catalog object
/// creation and this crate's `PDFA1B-CATALOG-001` gate: a `/Root` that is
/// missing, direct, unresolvable, or points at a non-`Catalog` (or untyped)
/// dictionary is reported as `catalog_present == false` rather than
/// silently treated as a usable catalog by some consumers and not others.
pub(crate) fn resolve_catalog<'a>(
    document: &'a Document,
    limits: &SafetyLimits,
) -> Result<Option<Catalog<'a>>, PdfError> {
    let Ok(root) = document.trailer.get(b"Root") else {
        return Ok(None);
    };
    let Some(object_id) = root.as_reference().ok().map(PdfObjectId::from) else {
        return Ok(None);
    };
    let dictionary = resolve_optional(document, root, limits.max_reference_depth)?
        .and_then(|object| object.as_dict().ok())
        .filter(|dictionary| dictionary.get_type().ok() == Some(b"Catalog".as_slice()));
    Ok(dictionary.map(|dictionary| Catalog {
        dictionary,
        object_id,
    }))
}

/// The trailer `/Root` entry's own indirect object id, whether or not it
/// resolves to a valid `/Type /Catalog` dictionary. `resolve_catalog` only
/// exposes an id once the catalog is fully valid; callers that need to
/// attribute a failure to "whatever `/Root` pointed at" even when the
/// catalog itself doesn't validate (`PDFA1B-CATALOG-001` and similar) use
/// this instead of independently re-deriving it from `document.trailer`.
pub(crate) fn root_reference_id(document: &Document) -> Option<PdfObjectId> {
    document
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|value| value.as_reference().ok())
        .map(Into::into)
}

#[cfg(test)]
mod tests {
    use lopdf::{Document, Object, dictionary};

    use super::resolve_catalog;
    use crate::SafetyLimits;

    #[test]
    fn rejects_a_root_pointing_at_a_non_catalog_dictionary() {
        let mut document = Document::with_version("1.4");
        let root_id = document.add_object(dictionary! { "Type" => "Other" });
        document.trailer.set("Root", Object::Reference(root_id));

        let catalog = resolve_catalog(&document, &SafetyLimits::default()).expect("resolve");
        assert!(catalog.is_none());
    }

    #[test]
    fn rejects_a_root_pointing_at_an_untyped_dictionary() {
        let mut document = Document::with_version("1.4");
        let root_id = document.add_object(dictionary! { "Pages" => Object::Null });
        document.trailer.set("Root", Object::Reference(root_id));

        let catalog = resolve_catalog(&document, &SafetyLimits::default()).expect("resolve");
        assert!(catalog.is_none());
    }

    #[test]
    fn accepts_a_root_pointing_at_a_catalog_dictionary() {
        let mut document = Document::with_version("1.4");
        let root_id = document.add_object(dictionary! { "Type" => "Catalog" });
        document.trailer.set("Root", Object::Reference(root_id));

        let catalog = resolve_catalog(&document, &SafetyLimits::default())
            .expect("resolve")
            .expect("catalog present");
        assert_eq!(catalog.object_id, root_id.into());
    }
}
