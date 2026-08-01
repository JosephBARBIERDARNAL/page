use lopdf::{Document, Object};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::object_resolution::{dictionary_based, resolve_optional};
use crate::report::RuleFailure;

/// Checks a resolved file-specification dictionary for the forbidden `/EF`
/// key (`PDFA1B-FILE-SPEC-EMBEDDED-FILE-001`, `ISO 19005-1:2005:6.1.11:1`).
///
/// veraPDF creates the same `CosFileSpecification` object, and applies this
/// same predicate, regardless of how the file specification is reached: the
/// catalog `Names/EmbeddedFiles` name tree, or a `GoToR`/`SubmitForm`
/// action's `/F` entry (confirmed against veraPDF 1.28.2: a `GoToR` and a
/// `SubmitForm` action each targeting a file specification with an `/EF`
/// entry, and no `Names` tree at all, both report exactly this rule). Every
/// reachability path shares this one check instead of re-deriving it.
///
/// A direct `Object::Null` `/EF` value is empty and passes; an *indirect*
/// null remains a present key and fails, matching the same convention
/// `PDFA1B-NAMES-EMBEDDED-FILES-001` uses for the enclosing `EmbeddedFiles`
/// entry.
///
/// PDF32000 has exactly two other file-specification sites, and both are
/// deliberately *not* wired to this check because the object carrying the
/// file specification is already unconditionally forbidden for PDF/A-1b by
/// a different rule, making a second (redundant) report of the same
/// underlying object pointless:
/// - a `FileAttachment` annotation's `/FS` entry: `FileAttachment` is not in
///   `PDFA1B-ANNOTATION-SUBTYPE-001`'s allowed subtype list, so the
///   annotation itself already fails before its file specification would
///   matter;
/// - a `Launch`/`GoToE` action's file specification: neither action type is
///   in `PDFA1B-ACTION-TYPE-001`'s allowed list, so the action itself
///   already fails the same way.
///
/// That leaves the `Names/EmbeddedFiles` tree and a `GoToR`/`SubmitForm`
/// action's `/F` as the only two reachability paths that need this check,
/// which is exactly what's wired up (`document_features.rs`, `actions.rs`).
pub(crate) fn inspect(
    document: &Document,
    value: &Object,
    limits: &SafetyLimits,
    context: &str,
) -> Result<Option<RuleFailure>, PdfError> {
    let object_id = value.as_reference().ok().map(Into::into);
    let Some(file_spec) =
        resolve_optional(document, value, limits.max_reference_depth)?.and_then(dictionary_based)
    else {
        return Ok(None);
    };
    if file_spec
        .get(b"EF")
        .is_ok_and(|value| !matches!(value, Object::Null))
    {
        Ok(Some(RuleFailure {
            object_id,
            description: format!("{context} contains an EF entry"),
        }))
    } else {
        Ok(None)
    }
}
