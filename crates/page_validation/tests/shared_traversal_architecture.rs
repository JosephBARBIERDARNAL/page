//! Regression tests for the goal's "downstream validators consume the same
//! catalog/page/resource graph rather than maintaining inconsistent
//! reachability rules" requirement: rather than asserting this in prose,
//! these tests scan the actual source tree and fail if any file outside the
//! canonical modules re-implements catalog or page-tree resolution.

use std::fs;
use std::path::Path;

const SRC_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn src_files() -> Vec<(String, String)> {
    let dir = Path::new(SRC_DIR).join("src");
    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|error| {
        panic!("read {}: {error}", dir.display());
    }) {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("file name")
            .to_owned();
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        files.push((file_name, content));
    }
    assert!(!files.is_empty(), "expected to find source files in src/");
    files
}

/// Lines that are themselves comments (`//`, `///`, `//!`) don't count as a
/// real re-implementation — e.g. a doc comment that mentions the pattern by
/// name while explaining why it was replaced.
fn non_comment_occurrences(content: &str, pattern: &str) -> usize {
    content
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains(pattern))
        .count()
}

#[test]
fn only_catalog_rs_resolves_the_trailer_root_entry() {
    for (file_name, content) in src_files() {
        if file_name == "catalog.rs" {
            continue;
        }
        let occurrences = non_comment_occurrences(&content, r#"trailer.get(b"Root")"#);
        assert_eq!(
            occurrences, 0,
            "{file_name} resolves the trailer /Root entry directly instead of going through \
             catalog::resolve_catalog / catalog::root_reference_id"
        );
    }
}

#[test]
fn only_page_tree_rs_walks_the_page_tree() {
    for (file_name, content) in src_files() {
        if file_name == "page_tree.rs" {
            continue;
        }
        for pattern in [".get_pages()", "PageTreeIter"] {
            let occurrences = non_comment_occurrences(&content, pattern);
            assert_eq!(
                occurrences, 0,
                "{file_name} contains {pattern:?}, re-implementing page-tree traversal instead \
                 of consuming page_tree::collect_pages's shared ordered PageEntry vector"
            );
        }
    }
}

/// Every consumer of the shared page list takes `PageEntry`, not raw object
/// IDs — a regression to the pre-refactor shape (which could not represent a
/// directly embedded Page dictionary) would show up as a page-list parameter
/// using `ObjectId` outside page_tree.rs.
#[test]
fn shared_pages_map_is_keyed_by_page_entry_not_a_raw_object_id() {
    for (file_name, content) in src_files() {
        if file_name == "page_tree.rs" {
            continue;
        }
        let occurrences = non_comment_occurrences(&content, "pages: &[ObjectId]");
        assert_eq!(
            occurrences, 0,
            "{file_name} declares a page list using raw ObjectIds; it should use \
             page_tree::PageEntry via the shared ordered vector so directly embedded pages \
             (see page_tree.rs's doc comment) are representable"
        );
    }
}
