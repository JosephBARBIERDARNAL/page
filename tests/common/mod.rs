use lopdf::{Dictionary, Document, Object, Stream, dictionary};

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

fn replace(bytes: &mut Vec<u8>, from: &str, to: &str) {
    let text = String::from_utf8(bytes.clone()).expect("XMP fixture is UTF-8");
    assert!(text.contains(from), "fixture does not contain {from:?}");
    *bytes = text.replace(from, to).into_bytes();
}

fn replace_first(bytes: &mut Vec<u8>, from: &str, to: &str) {
    let text = String::from_utf8(bytes.clone()).expect("XMP fixture is UTF-8");
    assert!(text.contains(from), "fixture does not contain {from:?}");
    *bytes = text.replacen(from, to, 1).into_bytes();
}

fn replace_last(bytes: &mut Vec<u8>, from: &str, to: &str) {
    let text = String::from_utf8(bytes.clone()).expect("XMP fixture is UTF-8");
    let index = text.rfind(from).expect("fixture occurrence");
    let mut result = text;
    result.replace_range(index..index + from.len(), to);
    *bytes = result.into_bytes();
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
