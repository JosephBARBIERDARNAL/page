use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object, Stream, dictionary};

/// Splits `actual` against `baseline` into (added, removed) sets.
pub fn rule_delta<T: Ord + Clone>(
    baseline: &BTreeSet<T>,
    actual: &BTreeSet<T>,
) -> (BTreeSet<T>, BTreeSet<T>) {
    (
        actual.difference(baseline).cloned().collect(),
        baseline.difference(actual).cloned().collect(),
    )
}

pub fn metadata_fixture(case: &str) -> Vec<u8> {
    let mut xmp = BASE_XMP.to_owned();
    let mut metadata_dictionary = dictionary! {
        "Type" => "Metadata",
        "Subtype" => "XML",
    };
    let mut include_metadata = true;
    let mut info = complete_info();
    let mut compress_metadata = false;

    match case {
        "baseline_b" => {}
        "accepted_a" => replace(
            &mut xmp,
            "pdfaid:conformance=\"B\"",
            "pdfaid:conformance=\"A\"",
        ),
        "missing_metadata" => include_metadata = false,
        "missing_type" => {
            metadata_dictionary.remove(b"Type");
        }
        "missing_subtype" => {
            metadata_dictionary.remove(b"Subtype");
        }
        "metadata_filter" => compress_metadata = true,
        "malformed_xmp" => xmp = b"<rdf:RDF>".to_vec(),
        "missing_identification" => {
            replace(&mut xmp, " pdfaid:part=\"1\" pdfaid:conformance=\"B\"", "");
        }
        "wrong_part" => replace(&mut xmp, "pdfaid:part=\"1\"", "pdfaid:part=\"2\""),
        "lowercase_conformance" => replace(
            &mut xmp,
            "pdfaid:conformance=\"B\"",
            "pdfaid:conformance=\"b\"",
        ),
        "duplicate_identification" => replace(
            &mut xmp,
            "</rdf:RDF>",
            "<rdf:Description pdfaid:part=\"2\" pdfaid:conformance=\"A\"/></rdf:RDF>",
        ),
        "title_mismatch" => info.set("Title", Object::string_literal("different")),
        "author_mismatch" => info.set("Author", Object::string_literal("different")),
        "subject_mismatch" => info.set("Subject", Object::string_literal("different")),
        "keywords_mismatch" => info.set("Keywords", Object::string_literal("different")),
        "creator_mismatch" => info.set("Creator", Object::string_literal("different")),
        "producer_mismatch" => info.set("Producer", Object::string_literal("different")),
        "creation_date_equivalent_offset" => replace(
            &mut xmp,
            "2026-07-27T12:30:45+02:00",
            "2026-07-27T10:30:45Z",
        ),
        "creation_date_mismatch" => {
            replace_first(
                &mut xmp,
                "2026-07-27T12:30:45+02:00",
                "2026-07-28T12:30:45+02:00",
            );
        }
        "creation_date_invalid" => {
            replace_first(&mut xmp, "2026-07-27T12:30:45+02:00", "not-a-date");
        }
        "mod_date_mismatch" => {
            replace_last(
                &mut xmp,
                "2026-07-27T12:30:45+02:00",
                "2026-07-28T12:30:45+02:00",
            );
        }
        "author_multiple" => replace(
            &mut xmp,
            "<rdf:li>Author</rdf:li>",
            "<rdf:li>Author</rdf:li><rdf:li>Second</rdf:li>",
        ),
        _ => panic!("unknown metadata fixture case {case}"),
    }

    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        }),
    );
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    if include_metadata {
        let mut stream = Stream::new(metadata_dictionary, xmp);
        if compress_metadata {
            stream.compress().expect("compress test metadata");
        }
        let metadata_id = document.add_object(stream);
        catalog.set("Metadata", metadata_id);
    }
    let intent_id = document.add_object(dictionary! {
        "Type" => "OutputIntent",
        "S" => "GTS_PDFA1",
        "OutputConditionIdentifier" => Object::string_literal("Test"),
    });
    catalog.set("OutputIntents", vec![Object::Reference(intent_id)]);
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(info);
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save metadata fixture");
    bytes
}

pub fn output_intent_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        }),
    );
    let metadata_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        BASE_XMP.to_vec(),
    ));
    let info_id = document.add_object(complete_info());

    let rgb = icc_header(*b"mntr", *b"RGB ", 2, 1);
    let output_intents = match case {
        "baseline" => single_profile_intent(&mut document, rgb.clone(), Some("GTS_PDFA1")),
        "no_output_intents" => None,
        "wrong_type_array" => Some(42.into()),
        "empty_array" => Some(Object::Array(Vec::new())),
        "non_dictionary_entries" => {
            let number_id = document.add_object(Object::Integer(7));
            Some(Object::Array(vec![
                5.into(),
                Object::Reference(number_id),
                Object::Null,
            ]))
        }
        "direct_intent_dictionary" => Some(Object::Array(vec![Object::Dictionary(
            output_intent_dictionary(
                Some(profile_reference(&mut document, rgb.clone())),
                Some("GTS_PDFA1"),
            ),
        )])),
        "missing_s" => single_profile_intent(&mut document, rgb.clone(), None),
        "wrong_s" => single_profile_intent(&mut document, rgb.clone(), Some("GTS_PDFX")),
        "missing_dest_output_profile" => single_intent(&mut document, None, Some("GTS_PDFA1")),
        "direct_wrong_type_profile" => {
            single_intent(&mut document, Some(7.into()), Some("GTS_PDFA1"))
        }
        "indirect_wrong_type_profile" => {
            let wrong_id = document.add_object(dictionary! {"Not" => "AStream"});
            single_intent(
                &mut document,
                Some(Object::Reference(wrong_id)),
                Some("GTS_PDFA1"),
            )
        }
        "truncated_profile" => single_profile_intent(&mut document, vec![0; 19], Some("GTS_PDFA1")),
        "class_prtr" => single_profile_intent(
            &mut document,
            icc_header(*b"prtr", *b"RGB ", 2, 1),
            Some("GTS_PDFA1"),
        ),
        "class_scnr" => single_profile_intent(
            &mut document,
            icc_header(*b"scnr", *b"RGB ", 2, 1),
            Some("GTS_PDFA1"),
        ),
        "color_cmyk" => single_profile_intent(
            &mut document,
            icc_header(*b"mntr", *b"CMYK", 2, 1),
            Some("GTS_PDFA1"),
        ),
        "color_gray" => single_profile_intent(
            &mut document,
            icc_header(*b"mntr", *b"GRAY", 2, 1),
            Some("GTS_PDFA1"),
        ),
        "color_lab" => single_profile_intent(
            &mut document,
            icc_header(*b"mntr", *b"Lab ", 2, 1),
            Some("GTS_PDFA1"),
        ),
        "version_2_15" => single_profile_intent(
            &mut document,
            icc_header(*b"mntr", *b"RGB ", 2, 15),
            Some("GTS_PDFA1"),
        ),
        "version_3" => single_profile_intent(
            &mut document,
            icc_header(*b"mntr", *b"RGB ", 3, 0),
            Some("GTS_PDFA1"),
        ),
        "large_compressed_profile" => {
            let mut bytes = rgb.clone();
            bytes.resize(4096, 0);
            let profile = compressed_profile_reference(&mut document, bytes);
            single_intent(&mut document, Some(profile), Some("GTS_PDFA1"))
        }
        "two_shared_indirect_profiles" => {
            let profile = profile_reference(&mut document, rgb.clone());
            two_intents(&mut document, profile.clone(), profile)
        }
        "two_shared_invalid_profiles" => {
            let profile = profile_reference(&mut document, icc_header(*b"scnr", *b"RGB ", 2, 1));
            two_intents(&mut document, profile.clone(), profile)
        }
        "two_identical_indirect_profiles" => {
            let first = profile_reference(&mut document, rgb.clone());
            let second = profile_reference(&mut document, rgb.clone());
            two_intents(&mut document, first, second)
        }
        "two_different_indirect_profiles" => {
            let first = profile_reference(&mut document, rgb.clone());
            let second = profile_reference(&mut document, icc_header(*b"mntr", *b"CMYK", 2, 1));
            two_intents(&mut document, first, second)
        }
        "one_profile_one_missing" => {
            let profile = profile_reference(&mut document, rgb.clone());
            let first =
                document.add_object(output_intent_dictionary(Some(profile), Some("GTS_PDFA1")));
            let second = document.add_object(output_intent_dictionary(None, Some("GTS_PDFA1")));
            Some(Object::Array(vec![
                Object::Reference(first),
                Object::Reference(second),
            ]))
        }
        "two_same_wrong_type_indirect_profiles" => {
            let wrong = document.add_object(dictionary! {"Not" => "AStream"});
            two_intents(
                &mut document,
                Object::Reference(wrong),
                Object::Reference(wrong),
            )
        }
        "two_different_wrong_type_indirect_profiles" => {
            let first = document.add_object(dictionary! {"Not" => "AStream"});
            let second = document.add_object(dictionary! {"StillNot" => "AStream"});
            two_intents(
                &mut document,
                Object::Reference(first),
                Object::Reference(second),
            )
        }
        _ => panic!("unknown output-intent fixture case {case}"),
    };

    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
    };
    if let Some(output_intents) = output_intents {
        catalog.set("OutputIntents", output_intents);
    }
    let catalog_id = document.add_object(catalog);
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save output-intent fixture");
    bytes
}

fn single_intent(
    document: &mut Document,
    profile: Option<Object>,
    subtype: Option<&str>,
) -> Option<Object> {
    let intent_id = document.add_object(output_intent_dictionary(profile, subtype));
    Some(Object::Array(vec![Object::Reference(intent_id)]))
}

fn single_profile_intent(
    document: &mut Document,
    profile: Vec<u8>,
    subtype: Option<&str>,
) -> Option<Object> {
    let profile = profile_reference(document, profile);
    single_intent(document, Some(profile), subtype)
}

fn two_intents(document: &mut Document, first: Object, second: Object) -> Option<Object> {
    let first = document.add_object(output_intent_dictionary(Some(first), Some("GTS_PDFA1")));
    let second = document.add_object(output_intent_dictionary(Some(second), Some("GTS_PDFA1")));
    Some(Object::Array(vec![
        Object::Reference(first),
        Object::Reference(second),
    ]))
}

fn output_intent_dictionary(profile: Option<Object>, subtype: Option<&str>) -> Dictionary {
    let mut dictionary = dictionary! {
        "Type" => "OutputIntent",
        "OutputConditionIdentifier" => Object::string_literal("Test"),
    };
    if let Some(subtype) = subtype {
        dictionary.set("S", Object::Name(subtype.as_bytes().to_vec()));
    }
    if let Some(profile) = profile {
        dictionary.set("DestOutputProfile", profile);
    }
    dictionary
}

fn profile_reference(document: &mut Document, bytes: Vec<u8>) -> Object {
    Object::Reference(document.add_object(profile_stream(bytes)))
}

fn compressed_profile_reference(document: &mut Document, bytes: Vec<u8>) -> Object {
    let mut stream = profile_stream(bytes);
    stream.compress().expect("compress ICC test profile");
    Object::Reference(document.add_object(stream))
}

fn profile_stream(bytes: Vec<u8>) -> Stream {
    let components = bytes.get(16..20).and_then(|signature| match signature {
        b"GRAY" => Some(1),
        b"RGB " | b"Lab " => Some(3),
        b"CMYK" => Some(4),
        _ => None,
    });
    let mut dictionary = Dictionary::new();
    if let Some(components) = components {
        dictionary.set("N", components);
    }
    Stream::new(dictionary, bytes)
}

fn icc_header(
    device_class: [u8; 4],
    color_space: [u8; 4],
    version_major: u8,
    version_minor: u8,
) -> Vec<u8> {
    let mut bytes = vec![0; 20];
    bytes[0..4].copy_from_slice(&20u32.to_be_bytes());
    bytes[8] = version_major;
    bytes[9] = version_minor << 4;
    bytes[12..16].copy_from_slice(&device_class);
    bytes[16..20].copy_from_slice(&color_space);
    bytes
}

fn complete_info() -> Dictionary {
    dictionary! {
        "Title" => Object::string_literal("Title"),
        "Author" => Object::string_literal("Author"),
        "Subject" => Object::string_literal("Subject"),
        "Keywords" => Object::string_literal("rust,pdf"),
        "Creator" => Object::string_literal("tool"),
        "Producer" => Object::string_literal("producer"),
        "CreationDate" => Object::string_literal("D:20260727123045+02'00'"),
        "ModDate" => Object::string_literal("D:20260727123045+02'00'"),
    }
}

enum Occurrence {
    All,
    First,
    Last,
}

fn replace(bytes: &mut Vec<u8>, from: &str, to: &str) {
    replace_occurrence(bytes, from, to, Occurrence::All);
}

fn replace_first(bytes: &mut Vec<u8>, from: &str, to: &str) {
    replace_occurrence(bytes, from, to, Occurrence::First);
}

fn replace_last(bytes: &mut Vec<u8>, from: &str, to: &str) {
    replace_occurrence(bytes, from, to, Occurrence::Last);
}

fn replace_occurrence(bytes: &mut Vec<u8>, from: &str, to: &str, occurrence: Occurrence) {
    let text = String::from_utf8(bytes.clone()).expect("XMP fixture is UTF-8");
    let replaced = match occurrence {
        Occurrence::All => {
            assert!(text.contains(from), "fixture does not contain {from:?}");
            text.replace(from, to)
        }
        Occurrence::First => {
            assert!(text.contains(from), "fixture does not contain {from:?}");
            text.replacen(from, to, 1)
        }
        Occurrence::Last => {
            let index = text.rfind(from).expect("fixture occurrence");
            let mut result = text;
            result.replace_range(index..index + from.len(), to);
            result
        }
    };
    *bytes = replaced.into_bytes();
}

const BASE_XMP: &[u8] = br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
 xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/"
 xmlns:dc="http://purl.org/dc/elements/1.1/"
 xmlns:pdf="http://ns.adobe.com/pdf/1.3/"
 xmlns:xmp="http://ns.adobe.com/xap/1.0/">
<rdf:Description pdfaid:part="1" pdfaid:conformance="B"
 pdf:Keywords="rust,pdf" pdf:Producer="producer"
 xmp:CreatorTool="tool" xmp:CreateDate="2026-07-27T12:30:45+02:00"
 xmp:ModifyDate="2026-07-27T12:30:45+02:00">
<dc:title><rdf:Alt><rdf:li xml:lang="fr">Titre</rdf:li>
<rdf:li xml:lang="x-default">Title</rdf:li></rdf:Alt></dc:title>
<dc:creator><rdf:Seq><rdf:li>Author</rdf:li></rdf:Seq></dc:creator>
<dc:description><rdf:Alt><rdf:li xml:lang="x-default">Subject</rdf:li>
</rdf:Alt></dc:description>
</rdf:Description>
</rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;
