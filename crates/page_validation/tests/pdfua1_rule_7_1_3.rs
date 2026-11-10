pub mod common;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

const RULE: &str = "ISO 14289-1:2014:7.1:3";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-3-artifact.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-3-artifact.pdf").to_vec(),
            || common::pdfua1_rule_7_1_3_fixture("artifact"),
            &[],
        ),
        (
            "pdfua1-rule-7-1-3-tagged.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-3-tagged.pdf").to_vec(),
            || common::pdfua1_rule_7_1_3_fixture("tagged"),
            &[],
        ),
        (
            "pdfua1-rule-7-1-3-untagged.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-3-untagged.pdf").to_vec(),
            || common::pdfua1_rule_7_1_3_fixture("untagged"),
            &["ISO 14289-1:2014:7.1:3"],
        ),
    ],
}

fn tagged_content_document(content: &[u8]) -> (Document, ObjectId, ObjectId, ObjectId) {
    let mut document = common::pdf_document();
    let pages = document.new_object_id();
    let root = document.new_object_id();
    let stream = document.add_object(Stream::new(Dictionary::new(), content.to_vec()));
    let page = document.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages, "Contents" => stream,
        "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
        "StructParents" => 0,
    });
    let element = document.add_object(dictionary! {
        "Type" => "StructElem", "S" => "P", "P" => root, "Pg" => page, "K" => 0,
    });
    document.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1,
        }),
    );
    document.objects.insert(
        root,
        Object::Dictionary(dictionary! {
            "Type" => "StructTreeRoot", "K" => vec![Object::Reference(element)],
            "ParentTree" => dictionary! {
                "Nums" => vec![0.into(), Object::Array(vec![element.into()])],
            },
        }),
    );
    let catalog = document.add_object(dictionary! {
        "Type" => "Catalog", "Pages" => pages, "StructTreeRoot" => root,
        "Lang" => Object::string_literal("en-US"),
        "MarkInfo" => dictionary! { "Marked" => true },
    });
    document.trailer.set("Root", catalog);
    (document, page, root, element)
}

fn assert_untagged(document: &mut Document, expected: bool, case: &str) {
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save tagging regression");
    let report = validate_pdf_bytes(
        &bytes,
        &ValidationOptions::default().profile(ValidationProfile::PdfUa1),
    )
    .expect("validate tagging regression");
    assert_eq!(
        report
            .failures
            .iter()
            .any(|failure| failure.rule_id == RULE),
        expected,
        "{case}: {report:#?}"
    );
}

#[test]
fn mcid_content_requires_a_connected_structural_parent() {
    for case in [
        "connected",
        "no_root",
        "no_parent_tree",
        "no_struct_parents",
        "missing_key",
        "missing_mcid",
        "null_mcid",
        "missing_parent",
        "invalid_parent",
        "cyclic_parent",
        "wrong_root",
    ] {
        let (mut document, page, root, element) =
            tagged_content_document(b"/P <</MCID 0>> BDC 0 0 m 10 10 l S EMC");
        match case {
            "no_root" => {
                document
                    .catalog_mut()
                    .expect("catalog")
                    .remove(b"StructTreeRoot");
            }
            "no_parent_tree" => {
                document
                    .get_dictionary_mut(root)
                    .expect("root")
                    .remove(b"ParentTree");
            }
            "no_struct_parents" => {
                document
                    .get_dictionary_mut(page)
                    .expect("page")
                    .remove(b"StructParents");
            }
            "missing_key" => document
                .get_dictionary_mut(page)
                .expect("page")
                .set("StructParents", 4),
            "missing_mcid" | "null_mcid" => {
                let entries = if case == "null_mcid" {
                    vec![Object::Null]
                } else {
                    Vec::new()
                };
                document.get_dictionary_mut(root).expect("root").set(
                    "ParentTree",
                    dictionary! {
                        "Nums" => vec![0.into(), Object::Array(entries)],
                    },
                );
            }
            "missing_parent" => {
                document
                    .get_dictionary_mut(element)
                    .expect("element")
                    .remove(b"P");
            }
            "invalid_parent" => document
                .get_dictionary_mut(element)
                .expect("element")
                .set("P", 42),
            "cyclic_parent" => document
                .get_dictionary_mut(element)
                .expect("element")
                .set("P", element),
            "wrong_root" => {
                let other = document.add_object(dictionary! { "Type" => "StructTreeRoot" });
                document
                    .get_dictionary_mut(element)
                    .expect("element")
                    .set("P", other);
            }
            _ => {}
        }
        assert_untagged(&mut document, case != "connected", case);
    }
}

#[test]
fn artifacts_and_unmapped_nested_mcid_preserve_inherited_tagging() {
    for (content, fails) in [
        (
            "/Artifact BMC /P <</MCID 4>> BDC 0 0 m 10 10 l S EMC EMC",
            false,
        ),
        (
            "/P <</MCID 0>> BDC /Span <</MCID 4>> BDC 0 0 m 10 10 l S EMC EMC",
            false,
        ),
        ("/P <</MCID 4>> BDC 0 0 m 10 10 l S EMC", true),
    ] {
        let (mut document, ..) = tagged_content_document(content.as_bytes());
        assert_untagged(&mut document, fails, content);
    }
}

#[test]
fn forms_use_their_own_parent_tree_and_inherit_the_invocation_context() {
    for (content, form_content, mapped, fails) in [
        (
            "/P <</MCID 0>> BDC /F Do EMC",
            "0 0 m 10 10 l S",
            false,
            false,
        ),
        (
            "/P <</MCID 4>> BDC /F Do EMC",
            "0 0 m 10 10 l S",
            false,
            true,
        ),
        (
            "/P <</MCID 0>> BDC /F Do EMC /F Do",
            "0 0 m 10 10 l S",
            false,
            true,
        ),
        (
            "/F Do /P <</MCID 0>> BDC /F Do EMC",
            "0 0 m 10 10 l S",
            false,
            true,
        ),
        ("/Artifact BMC /F Do EMC", "0 0 m 10 10 l S", false, false),
        (
            "/F Do",
            "/P <</MCID 0>> BDC 0 0 m 10 10 l S EMC",
            false,
            true,
        ),
        (
            "/F Do",
            "/P <</MCID 0>> BDC 0 0 m 10 10 l S EMC",
            true,
            false,
        ),
    ] {
        let (mut document, page, root, element) = tagged_content_document(content.as_bytes());
        let mut form_dict = dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
            "Resources" => Dictionary::new(),
        };
        if mapped {
            form_dict.set("StructParents", 1);
            document.get_dictionary_mut(root).expect("root").set("ParentTree", dictionary! {
                "Nums" => vec![0.into(), Object::Array(vec![element.into()]), 1.into(), Object::Array(vec![element.into()])],
            });
        }
        let form = document.add_object(Stream::new(form_dict, form_content.as_bytes().to_vec()));
        document.get_dictionary_mut(page).expect("page").set(
            "Resources",
            dictionary! {
                "XObject" => dictionary! { "F" => form },
            },
        );
        assert_untagged(&mut document, fails, content);
    }
}
