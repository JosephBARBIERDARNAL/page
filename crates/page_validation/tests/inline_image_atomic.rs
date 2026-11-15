use lopdf::{Dictionary, Object, Stream, dictionary};
use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

pub mod common;

#[test]
fn inline_images_and_masks_preserve_ordinary_validation_in_all_affected_profiles() {
    let ordinary = b"1 0 0 rg BT /F1 12 Tf 20 100 Td (A) Tj ET\n";
    for image in [
        b"q BI /W 1 /H 1 /CS /RGB /BPC 8 ID \xff\x00\x00 EI Q\n".as_slice(),
        b"q BI /W 1 /H 1 /IM true /BPC 1 ID \x80 EI Q\n".as_slice(),
    ] {
        for image_first in [true, false] {
            let content = if image_first {
                [image, ordinary].concat()
            } else {
                [ordinary.as_slice(), image].concat()
            };
            let bytes = inline_image_document(&content);
            for (profile, expected_rules) in [
                (
                    ValidationProfile::PdfA1b,
                    ["ISO 19005-1:2005:6.3.4:1", "ISO 19005-1:2005:6.2.3.3:1"].as_slice(),
                ),
                (
                    ValidationProfile::PdfA2b,
                    [
                        "ISO 19005-2:2011:6.2.11.4.1:1",
                        "ISO 19005-2:2011:6.2.4.3:2",
                    ]
                    .as_slice(),
                ),
                (
                    ValidationProfile::PdfUa1,
                    [
                        "ISO 14289-1:2014:7.21.4.1:1",
                        "ISO 14289-1:2014:7.1:3",
                        "ISO 14289-1:2014:7.2:34",
                    ]
                    .as_slice(),
                ),
            ] {
                let report =
                    validate_pdf_bytes(&bytes, &ValidationOptions::default().profile(profile))
                        .expect("validate inline image regression");
                for expected_rule in expected_rules {
                    assert!(
                        report
                            .failures
                            .iter()
                            .any(|failure| failure.rule_id == *expected_rule),
                        "{profile:?}, image first={image_first}: missing {expected_rule}: {report:#?}"
                    );
                }
            }
        }
    }
}

fn inline_image_document(content: &[u8]) -> Vec<u8> {
    let mut document = common::pdf_document();
    let pages = document.new_object_id();
    let font = document.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let stream = document.add_object(Stream::new(Dictionary::new(), content.to_vec()));
    let page = document.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages, "Contents" => stream,
        "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
    });
    document.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1,
        }),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    document.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save inline image PDF");
    bytes
}
