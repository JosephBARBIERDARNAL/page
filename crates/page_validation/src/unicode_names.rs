use std::collections::BTreeSet;

use lopdf::{Document, Object, ObjectId};

use crate::content_support::inherited_page_resources;
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{dictionary_based, resolve_optional, resolved_name};
use crate::page_tree::PageEntry;
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct UnicodeNameSummary {
    pub(crate) failures: Vec<RuleFailure>,
}

pub(crate) fn is_valid_utf8(name: &[u8]) -> bool {
    std::str::from_utf8(name).is_ok()
}

pub(crate) fn inspect(
    document: &Document,
    pages: &[PageEntry],
    limits: &SafetyLimits,
) -> Result<UnicodeNameSummary, PdfError> {
    let mut failures = Vec::new();
    let mut visited = BTreeSet::new();
    for page in pages {
        let Some(page_dictionary) = page.resolve(document) else {
            continue;
        };
        let Some(resources) = inherited_page_resources(document, page_dictionary, limits)? else {
            continue;
        };
        inspect_object(
            document,
            &Object::Dictionary(resources.clone()),
            page.object_id().map(Into::into),
            limits,
            0,
            &mut visited,
            &mut failures,
        )?;
    }
    failures.sort_by(|left, right| {
        left.object_id
            .cmp(&right.object_id)
            .then_with(|| left.description.cmp(&right.description))
    });
    failures.dedup_by(|left, right| {
        left.object_id == right.object_id && left.description == right.description
    });
    Ok(UnicodeNameSummary { failures })
}

fn inspect_object(
    document: &Document,
    object: &Object,
    owner: Option<PdfObjectId>,
    limits: &SafetyLimits,
    depth: usize,
    visited: &mut BTreeSet<ObjectId>,
    failures: &mut Vec<RuleFailure>,
) -> Result<(), PdfError> {
    if depth > limits.max_reference_depth {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    match object {
        Object::Reference(id) => {
            if !visited.insert(*id) {
                return Ok(());
            }
            if let Some(resolved) = resolve_optional(document, object, limits.max_reference_depth)?
            {
                inspect_object(
                    document,
                    resolved,
                    Some((*id).into()),
                    limits,
                    depth + 1,
                    visited,
                    failures,
                )?;
            }
        }
        Object::Array(items) => {
            inspect_color_space(document, items, owner, limits, failures)?;
            for item in items {
                inspect_object(document, item, owner, limits, depth + 1, visited, failures)?;
            }
        }
        _ => {
            if let Some(dictionary) = dictionary_based(object) {
                if resolved_name(document, dictionary, b"Type", limits.max_reference_depth)?
                    == Some(b"Font".as_slice())
                    && let Some(name) = resolved_name(
                        document,
                        dictionary,
                        b"BaseFont",
                        limits.max_reference_depth,
                    )?
                    && !is_valid_utf8(name)
                {
                    failures.push(RuleFailure {
                        object_id: owner,
                        description: "a font /BaseFont name is not valid UTF-8".to_owned(),
                    });
                }
                for (_, value) in dictionary.iter() {
                    inspect_object(document, value, owner, limits, depth + 1, visited, failures)?;
                }
            }
        }
    }
    Ok(())
}

fn inspect_color_space(
    document: &Document,
    items: &[Object],
    owner: Option<PdfObjectId>,
    limits: &SafetyLimits,
    failures: &mut Vec<RuleFailure>,
) -> Result<(), PdfError> {
    let Some(kind) = items.first().and_then(|item| item.as_name().ok()) else {
        return Ok(());
    };
    match kind {
        b"Separation" => inspect_name(
            document,
            items.get(1),
            owner,
            limits,
            failures,
            "Separation colourant",
        )?,
        b"DeviceN" => {
            let Some(names) = items
                .get(1)
                .map(|value| resolve_optional(document, value, limits.max_reference_depth))
                .transpose()?
                .flatten()
                .and_then(|value| value.as_array().ok())
            else {
                return Ok(());
            };
            for name in names {
                inspect_name(
                    document,
                    Some(name),
                    owner,
                    limits,
                    failures,
                    "DeviceN colourant",
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn inspect_name(
    document: &Document,
    value: Option<&Object>,
    owner: Option<PdfObjectId>,
    limits: &SafetyLimits,
    failures: &mut Vec<RuleFailure>,
    context: &str,
) -> Result<(), PdfError> {
    if let Some(name) = value
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_name().ok())
        && !is_valid_utf8(name)
    {
        failures.push(RuleFailure {
            object_id: owner,
            description: format!("a {context} name is not valid UTF-8"),
        });
    }
    Ok(())
}
