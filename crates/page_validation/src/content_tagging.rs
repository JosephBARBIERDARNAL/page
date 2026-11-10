//! Resolves marked content through the page or Form's ParentTree entry and checks that its
//! structural parent chain reaches the catalog's structure root. Number-tree lookups and MCID
//! connections are cached for one content execution; bounded walks detect cycles and enforce
//! the existing reference-depth and object-count limits.

use std::collections::{BTreeSet, HashMap};

use lopdf::{Dictionary, Document, Object};

use crate::annotations::find_number_tree_entry;
use crate::catalog::resolve_catalog;
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::object_resolution::resolve_optional;

pub(crate) struct ContentTagging<'a> {
    root: Option<&'a Dictionary>,
    parents: HashMap<i64, Option<&'a [Object]>>,
    connections: HashMap<(i64, i64), Option<bool>>,
}

impl<'a> ContentTagging<'a> {
    pub(crate) fn new(document: &'a Document, limits: &SafetyLimits) -> Result<Self, PdfError> {
        let root = resolve_catalog(document, limits)?
            .and_then(|catalog| catalog.dictionary.get(b"StructTreeRoot").ok())
            .map(|value| resolve_optional(document, value, limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|value| value.as_dict().ok());
        Ok(Self {
            root,
            parents: HashMap::new(),
            connections: HashMap::new(),
        })
    }

    /// `None` leaves the enclosing structural context in effect; a mapped but
    /// disconnected parent overrides it with `Some(false)`.
    pub(crate) fn connection(
        &mut self,
        document: &'a Document,
        limits: &SafetyLimits,
        struct_parents: Option<i64>,
        mcid: i64,
    ) -> Result<Option<bool>, PdfError> {
        let (Some(root), Some(struct_parents), Ok(index)) =
            (self.root, struct_parents, usize::try_from(mcid))
        else {
            return Ok(None);
        };
        if let Some(connection) = self.connections.get(&(struct_parents, mcid)) {
            return Ok(*connection);
        }
        let parents = if let Some(parents) = self.parents.get(&struct_parents) {
            *parents
        } else {
            let parent_tree = root
                .get(b"ParentTree")
                .ok()
                .map(|value| resolve_optional(document, value, limits.max_reference_depth))
                .transpose()?
                .flatten()
                .and_then(|value| value.as_dict().ok());
            let entry = if let Some(parent_tree) = parent_tree {
                find_number_tree_entry(
                    document,
                    parent_tree,
                    struct_parents,
                    limits,
                    &mut BTreeSet::new(),
                    &mut 0,
                    0,
                )?
            } else {
                None
            };
            let parents = entry
                .map(|value| resolve_optional(document, value, limits.max_reference_depth))
                .transpose()?
                .flatten()
                .and_then(|value| value.as_array().ok())
                .map(Vec::as_slice);
            self.parents.insert(struct_parents, parents);
            parents
        };
        let connection = if let Some(parent) = parents.and_then(|parents| parents.get(index)) {
            let parent = resolve_optional(document, parent, limits.max_reference_depth)?
                .and_then(|value| value.as_dict().ok());
            parent
                .map(|parent| reaches_root(document, parent, root, limits))
                .transpose()?
        } else {
            None
        };
        self.connections.insert((struct_parents, mcid), connection);
        Ok(connection)
    }
}

fn reaches_root<'a>(
    document: &'a Document,
    mut parent: &'a Dictionary,
    root: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let mut visited = BTreeSet::new();
    for depth in 0..=limits.max_reference_depth {
        if std::ptr::eq(parent, root) {
            return Ok(true);
        }
        let Ok(next) = parent.get(b"P") else {
            return Ok(false);
        };
        if let Ok(id) = next.as_reference()
            && !visited.insert(id)
        {
            return Ok(false);
        }
        if depth >= limits.max_object_count {
            return Err(PdfError::TooManyObjects {
                actual: depth + 1,
                limit: limits.max_object_count,
            });
        }
        let Some(next) = resolve_optional(document, next, limits.max_reference_depth)?
            .and_then(|value| value.as_dict().ok())
        else {
            return Ok(false);
        };
        parent = next;
    }
    Err(PdfError::ReferenceDepth(limits.max_reference_depth))
}

#[cfg(test)]
mod tests {
    use lopdf::{Document, Object, dictionary};

    use super::{ContentTagging, reaches_root};
    use crate::{PdfError, SafetyLimits};

    #[test]
    fn parent_lookups_cache_connections_and_missing_entries() {
        let mut document = Document::with_version("1.7");
        let root_id = document.new_object_id();
        let parent_id = document.add_object(dictionary! { "P" => root_id });
        let kid = document.add_object(dictionary! {
            "Nums" => vec![Object::Integer(2), Object::Array(vec![parent_id.into()])],
        });
        document.objects.insert(
            root_id,
            Object::Dictionary(dictionary! {
                "Type" => "StructTreeRoot",
                "ParentTree" => dictionary! { "Kids" => vec![Object::Reference(kid)] },
            }),
        );
        let catalog = document.add_object(dictionary! {
            "Type" => "Catalog", "StructTreeRoot" => root_id,
        });
        document.trailer.set("Root", catalog);
        let limits = SafetyLimits::default();
        let mut tagging = ContentTagging::new(&document, &limits).expect("tagging index");
        for _ in 0..2 {
            assert_eq!(
                tagging
                    .connection(&document, &limits, Some(2), 0)
                    .expect("mapped"),
                Some(true)
            );
            assert_eq!(
                tagging
                    .connection(&document, &limits, Some(2), 4)
                    .expect("orphan"),
                None
            );
            assert_eq!(
                tagging
                    .connection(&document, &limits, Some(3), 0)
                    .expect("missing"),
                None
            );
        }
        assert_eq!(tagging.parents.len(), 2);
        assert_eq!(tagging.connections.len(), 3);
    }

    #[test]
    fn parent_walk_obeys_depth_and_object_limits_and_detects_cycles() {
        let mut document = Document::with_version("1.7");
        let root = dictionary! { "Type" => "StructTreeRoot" };
        let parent_id = document.add_object(dictionary! {});
        let child = dictionary! { "P" => parent_id };
        assert!(matches!(
            reaches_root(
                &document,
                &child,
                &root,
                &SafetyLimits::default().with_max_reference_depth(0)
            ),
            Err(PdfError::ReferenceDepth(0))
        ));
        assert!(matches!(
            reaches_root(
                &document,
                &child,
                &root,
                &SafetyLimits::default().with_max_object_count(0)
            ),
            Err(PdfError::TooManyObjects { .. })
        ));
        document.objects.insert(
            parent_id,
            Object::Dictionary(dictionary! { "P" => parent_id }),
        );
        assert!(
            !reaches_root(&document, &child, &root, &SafetyLimits::default())
                .expect("cycle is disconnected")
        );
    }
}
