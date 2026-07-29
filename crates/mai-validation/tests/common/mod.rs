use std::collections::BTreeSet;

use lopdf::content::{Content, Operation};
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

    if case.starts_with("extension_") {
        let replacement = format!("{EXTENSION_SCHEMA_BLOCK}</rdf:RDF>");
        replace(&mut xmp, "</rdf:RDF>", &replacement);
    }
    if case.starts_with("id_") {
        replace(
            &mut xmp,
            "xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\"",
            "xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\"\n xmlns:idAlias=\"http://www.aiim.org/pdfa/ns/id/\"",
        );
    }

    match case {
        "baseline_b" => {}
        "id_alias_declaration_only" => {}
        "id_part_alias" => replace(&mut xmp, "pdfaid:part=\"1\"", "idAlias:part=\"1\""),
        "id_conformance_alias" => replace(
            &mut xmp,
            "pdfaid:conformance=\"B\"",
            "idAlias:conformance=\"B\"",
        ),
        "id_amd_canonical" => replace(
            &mut xmp,
            "pdfaid:part=\"1\"",
            "pdfaid:amd=\"1:2005\" pdfaid:part=\"1\"",
        ),
        "id_amd_alias" => replace(
            &mut xmp,
            "pdfaid:part=\"1\"",
            "idAlias:amd=\"1:2005\" pdfaid:part=\"1\"",
        ),
        "id_part_default_element" => {
            replace(&mut xmp, " pdfaid:part=\"1\"", "");
            replace(
                &mut xmp,
                "<dc:title>",
                "<part xmlns=\"http://www.aiim.org/pdfa/ns/id/\">1</part><dc:title>",
            );
        }
        "id_conformance_default_element" => {
            replace(&mut xmp, " pdfaid:conformance=\"B\"", "");
            replace(
                &mut xmp,
                "<dc:title>",
                "<conformance xmlns=\"http://www.aiim.org/pdfa/ns/id/\">B</conformance><dc:title>",
            );
        }
        "id_amd_default_element" => replace(
            &mut xmp,
            "<dc:title>",
            "<amd xmlns=\"http://www.aiim.org/pdfa/ns/id/\">1:2005</amd><dc:title>",
        ),
        "extension_valid" => {}
        "extension_undefined_field" => replace(
            &mut xmp,
            "<pdfaSchema:schema>Example schema</pdfaSchema:schema>",
            "<pdfaSchema:schema>Example schema</pdfaSchema:schema><pdfaSchema:unknown>bad</pdfaSchema:unknown>",
        ),
        "extension_container_prefix" => {
            replace(
                &mut xmp,
                "<pdfaExtension:schemas>",
                "<extensionAlias:schemas>",
            );
            replace(
                &mut xmp,
                "</pdfaExtension:schemas>",
                "</extensionAlias:schemas>",
            );
        }
        "extension_container_seq" => {
            replace(&mut xmp, "<rdf:Bag>", "<rdf:Seq>");
            replace(&mut xmp, "</rdf:Bag>", "</rdf:Seq>");
        }
        "extension_schema_name_prefix" => {
            replace(&mut xmp, "<pdfaSchema:schema>", "<schemaAlias:schema>");
            replace(&mut xmp, "</pdfaSchema:schema>", "</schemaAlias:schema>");
        }
        "extension_schema_namespace_prefix" => {
            replace(
                &mut xmp,
                "<pdfaSchema:namespaceURI>http://example.com/ns/</pdfaSchema:namespaceURI>",
                "<schemaAlias:namespaceURI>http://example.com/ns/</schemaAlias:namespaceURI>",
            );
        }
        "extension_schema_prefix_prefix" => {
            replace(
                &mut xmp,
                "<pdfaSchema:prefix>ex</pdfaSchema:prefix>",
                "<schemaAlias:prefix>ex</schemaAlias:prefix>",
            );
        }
        "extension_property_bag" => {
            replace(
                &mut xmp,
                "<pdfaSchema:property><rdf:Seq>",
                "<pdfaSchema:property><rdf:Bag>",
            );
            replace(
                &mut xmp,
                "</rdf:Seq></pdfaSchema:property>",
                "</rdf:Bag></pdfaSchema:property>",
            );
        }
        "extension_value_type_bag" => {
            replace(
                &mut xmp,
                "<pdfaSchema:valueType><rdf:Seq>",
                "<pdfaSchema:valueType><rdf:Bag>",
            );
            replace(
                &mut xmp,
                "</rdf:Seq></pdfaSchema:valueType>",
                "</rdf:Bag></pdfaSchema:valueType>",
            );
        }
        "extension_property_name_prefix" => {
            replace(
                &mut xmp,
                "<pdfaProperty:name>example</pdfaProperty:name>",
                "<propertyAlias:name>example</propertyAlias:name>",
            );
        }
        "extension_property_value_type_prefix" => {
            replace(
                &mut xmp,
                "<pdfaProperty:valueType>Text</pdfaProperty:valueType>",
                "<propertyAlias:valueType>Text</propertyAlias:valueType>",
            );
        }
        "extension_property_unknown_value_type" => replace(
            &mut xmp,
            "<pdfaProperty:valueType>Text</pdfaProperty:valueType>",
            "<pdfaProperty:valueType>UnknownType</pdfaProperty:valueType>",
        ),
        "extension_property_category_prefix" => {
            replace(
                &mut xmp,
                "<pdfaProperty:category>external</pdfaProperty:category>",
                "<propertyAlias:category>external</propertyAlias:category>",
            );
        }
        "extension_property_bad_category" => replace(
            &mut xmp,
            "<pdfaProperty:category>external</pdfaProperty:category>",
            "<pdfaProperty:category>invalid</pdfaProperty:category>",
        ),
        "extension_property_description_prefix" => {
            replace(
                &mut xmp,
                "<pdfaProperty:description>Example property</pdfaProperty:description>",
                "<propertyAlias:description>Example property</propertyAlias:description>",
            );
        }
        "extension_value_type_name_prefix" => {
            replace(
                &mut xmp,
                "<pdfaType:type>CustomType</pdfaType:type>",
                "<typeAlias:type>CustomType</typeAlias:type>",
            );
        }
        "extension_value_type_namespace_prefix" => {
            replace(
                &mut xmp,
                "<pdfaType:namespaceURI>http://example.com/type/</pdfaType:namespaceURI>",
                "<typeAlias:namespaceURI>http://example.com/type/</typeAlias:namespaceURI>",
            );
        }
        "extension_value_type_prefix_prefix" => {
            replace(
                &mut xmp,
                "<pdfaType:prefix>extype</pdfaType:prefix>",
                "<typeAlias:prefix>extype</typeAlias:prefix>",
            );
        }
        "extension_value_type_description_prefix" => {
            replace(
                &mut xmp,
                "<pdfaType:description>Example type</pdfaType:description>",
                "<typeAlias:description>Example type</typeAlias:description>",
            );
        }
        "extension_field_bag" => {
            replace(
                &mut xmp,
                "<pdfaType:field><rdf:Seq>",
                "<pdfaType:field><rdf:Bag>",
            );
            replace(
                &mut xmp,
                "</rdf:Seq></pdfaType:field>",
                "</rdf:Bag></pdfaType:field>",
            );
        }
        "extension_field_name_prefix" => {
            replace(
                &mut xmp,
                "<pdfaField:name>member</pdfaField:name>",
                "<fieldAlias:name>member</fieldAlias:name>",
            );
        }
        "extension_field_value_type_prefix" => {
            replace(
                &mut xmp,
                "<pdfaField:valueType>Text</pdfaField:valueType>",
                "<fieldAlias:valueType>Text</fieldAlias:valueType>",
            );
        }
        "extension_field_unknown_value_type" => replace(
            &mut xmp,
            "<pdfaField:valueType>Text</pdfaField:valueType>",
            "<pdfaField:valueType>UnknownType</pdfaField:valueType>",
        ),
        "extension_field_description_prefix" => {
            replace(
                &mut xmp,
                "<pdfaField:description>Example member</pdfaField:description>",
                "<fieldAlias:description>Example member</fieldAlias:description>",
            );
        }
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
        "packet_bytes_double" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes=\"123\"?>",
        ),
        "packet_bytes_single" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes='123'?>",
        ),
        "packet_bytes_spaced" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes \t= \t\"123\"?>",
        ),
        "packet_encoding" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" encoding=\"UTF-8\"?>",
        ),
        "packet_bytes_and_encoding" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes=\"123\" encoding='UTF-8'?>",
        ),
        "packet_uppercase_bytes" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" Bytes=\"123\"?>",
        ),
        "packet_unquoted_bytes" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes=123?>",
        ),
        "packet_substring_bytes" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" mybytes=\"123\"?>",
        ),
        "packet_body_bytes" => replace(
            &mut xmp,
            "<rdf:Description pdfaid:part=",
            "<rdf:Description bytes=\"123\" pdfaid:part=",
        ),
        "packet_end_bytes" => replace(
            &mut xmp,
            "<?xpacket end=\"w\"?>",
            "<?xpacket end=\"w\" bytes=\"123\"?>",
        ),
        "packet_first_forbidden_then_clean" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes=\"123\"?><?xpacket begin=\"\"?>",
        ),
        "packet_clean_then_forbidden" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\"?><?xpacket begin=\"\" bytes=\"123\"?>",
        ),
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

pub fn icc_based_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let valid = icc_header(*b"mntr", *b"RGB ", 2, 1);
    let (class, color_space, version_major, version_minor) = match case {
        "class_prtr" => (*b"prtr", *b"RGB ", 2, 1),
        "class_mntr" => (*b"mntr", *b"RGB ", 2, 1),
        "class_scnr" => (*b"scnr", *b"RGB ", 2, 1),
        "class_spac" => (*b"spac", *b"RGB ", 2, 1),
        "color_rgb" => (*b"mntr", *b"RGB ", 2, 1),
        "color_cmyk" => (*b"mntr", *b"CMYK", 2, 1),
        "color_gray" => (*b"mntr", *b"GRAY", 2, 1),
        "color_lab" => (*b"mntr", *b"Lab ", 2, 1),
        "version_2_15" => (*b"mntr", *b"RGB ", 2, 15),
        "invalid_class"
        | "direct_profile"
        | "repeated_shared_invalid"
        | "two_invalid_profiles"
        | "form_used"
        | "form_unused_resource"
        | "form_unreferenced"
        | "nested_form_used"
        | "cyclic_form"
        | "image_used"
        | "image_unused_resource"
        | "image_unreferenced"
        | "image_mask_ignores_color_space"
        | "image_smask_used"
        | "image_mask_image_used"
        | "image_alternate_used"
        | "unused_resource"
        | "default_gray"
        | "default_rgb"
        | "default_cmyk"
        | "unused_default"
        | "form_parent_fallback"
        | "nested_form_page_fallback"
        | "inline_image_used"
        | "shading_used"
        | "indexed_base_used" => (*b"link", *b"RGB ", 2, 1),
        "invalid_color_space" => (*b"mntr", *b"XYZ ", 2, 1),
        "version_3" => (*b"mntr", *b"RGB ", 3, 0),
        _ => (*b"mntr", *b"RGB ", 2, 1),
    };
    let selected_bytes = if case == "truncated_profile" {
        vec![0; 19]
    } else {
        icc_header(class, color_space, version_major, version_minor)
    };
    let selected_profile = if case == "undecodable_profile" {
        let mut stream = Stream::new(dictionary! {"N" => 3}, b"not deflate data".to_vec());
        stream.dict.set("Filter", "FlateDecode");
        Object::Reference(document.add_object(stream))
    } else if case == "large_compressed_profile" {
        let mut bytes = valid.clone();
        bytes.resize(4096, 0);
        compressed_profile_reference(&mut document, bytes)
    } else if matches!(case, "missing_n" | "wrong_n" | "non_integer_n") {
        let mut dictionary = Dictionary::new();
        if case == "wrong_n" {
            dictionary.set("N", 4);
        } else if case == "non_integer_n" {
            dictionary.set("N", Object::Name(b"Three".to_vec()));
        }
        Object::Reference(document.add_object(Stream::new(dictionary, selected_bytes)))
    } else {
        profile_reference(&mut document, selected_bytes)
    };
    let indirect_space = Object::Array(vec![
        Object::Name(b"ICCBased".to_vec()),
        selected_profile.clone(),
    ]);
    let direct_space = Object::Array(vec![
        Object::Name(b"ICCBased".to_vec()),
        Object::Stream(profile_stream(icc_header(*b"link", *b"RGB ", 2, 1))),
    ]);

    let mut page_resources = Dictionary::new();
    let mut page_contents = b"/CS1 CS\n".to_vec();
    match case {
        "baseline"
        | "class_prtr"
        | "class_mntr"
        | "class_scnr"
        | "class_spac"
        | "color_rgb"
        | "color_cmyk"
        | "color_gray"
        | "color_lab"
        | "version_2_15"
        | "invalid_class"
        | "invalid_color_space"
        | "version_3"
        | "truncated_profile"
        | "undecodable_profile"
        | "large_compressed_profile"
        | "missing_n"
        | "wrong_n"
        | "non_integer_n"
        | "inherited_resources" => {
            page_resources.set("ColorSpace", dictionary! {"CS1" => indirect_space});
        }
        "direct_profile" => {
            page_resources.set("ColorSpace", dictionary! {"CS1" => direct_space});
        }
        "unused_resource" => {
            page_resources.set("ColorSpace", dictionary! {"CS1" => indirect_space});
            page_contents.clear();
        }
        "default_gray" | "default_rgb" | "default_cmyk" | "unused_default" => {
            let (name, content) = match case {
                "default_gray" => ("DefaultGray", b"0 g\n0 G\n".as_slice()),
                "default_rgb" => ("DefaultRGB", b"0 0 0 rg\n0 0 0 RG\n".as_slice()),
                "default_cmyk" => ("DefaultCMYK", b"0 0 0 0 k\n0 0 0 0 K\n".as_slice()),
                _ => ("DefaultRGB", b"".as_slice()),
            };
            page_resources.set(
                "ColorSpace",
                Dictionary::from_iter([(name.as_bytes(), indirect_space)]),
            );
            page_contents = content.to_vec();
        }
        "missing_profile" => {
            page_resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![Object::Name(b"ICCBased".to_vec())]),
                },
            );
        }
        "wrong_profile_type" => {
            page_resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![
                        Object::Name(b"ICCBased".to_vec()),
                        Object::Integer(7),
                    ]),
                },
            );
        }
        "repeated_shared_valid" | "repeated_shared_invalid" => {
            page_resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => indirect_space.clone(),
                    "CS2" => indirect_space,
                },
            );
            page_contents = b"/CS1 CS\n/CS2 cs\n".to_vec();
        }
        "two_invalid_profiles" => {
            let second = profile_reference(&mut document, icc_header(*b"mntr", *b"XYZ ", 2, 1));
            page_resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => indirect_space,
                    "CS2" => Object::Array(vec![
                        Object::Name(b"ICCBased".to_vec()),
                        second,
                    ]),
                },
            );
            page_contents = b"/CS1 CS\n/CS2 cs\n".to_vec();
        }
        "form_used" | "form_unused_resource" | "form_unreferenced" => {
            let form_contents = if case == "form_unused_resource" {
                Vec::new()
            } else {
                b"/CS1 CS\n".to_vec()
            };
            let form = Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "ColorSpace" => dictionary! {"CS1" => indirect_space},
                    },
                },
                form_contents,
            );
            let form_id = document.add_object(form);
            if case != "form_unreferenced" {
                page_resources.set("XObject", dictionary! {"Fm" => form_id});
            }
            page_contents = if case == "form_used" || case == "form_unused_resource" {
                b"/Fm Do\n".to_vec()
            } else {
                Vec::new()
            };
        }
        "nested_form_used" => {
            let inner_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "ColorSpace" => dictionary! {"CS1" => indirect_space},
                    },
                },
                b"/CS1 CS\n".to_vec(),
            ));
            let outer_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "XObject" => dictionary! {"Inner" => inner_id},
                    },
                },
                b"/Inner Do\n".to_vec(),
            ));
            page_resources.set("XObject", dictionary! {"Outer" => outer_id});
            page_contents = b"/Outer Do\n".to_vec();
        }
        "form_parent_fallback" => {
            let form_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "XObject" => Dictionary::new(),
                    },
                },
                b"/CS1 CS\n".to_vec(),
            ));
            page_resources.set("ColorSpace", dictionary! {"CS1" => indirect_space});
            page_resources.set("XObject", dictionary! {"Fm" => form_id});
            page_contents = b"/Fm Do\n".to_vec();
        }
        "nested_form_page_fallback" => {
            let inner_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => Dictionary::new(),
                },
                b"/CS1 CS\n".to_vec(),
            ));
            let outer_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "XObject" => dictionary! {"Inner" => inner_id},
                    },
                },
                b"/Inner Do\n".to_vec(),
            ));
            page_resources.set("ColorSpace", dictionary! {"CS1" => indirect_space});
            page_resources.set("XObject", dictionary! {"Outer" => outer_id});
            page_contents = b"/Outer Do\n".to_vec();
        }
        "cyclic_form" => {
            let form_id = document.new_object_id();
            document.objects.insert(
                form_id,
                Object::Stream(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                        "Resources" => dictionary! {
                            "ColorSpace" => dictionary! {"CS1" => indirect_space},
                            "XObject" => dictionary! {"Self" => form_id},
                        },
                    },
                    b"/CS1 CS\n/Self Do\n".to_vec(),
                )),
            );
            page_resources.set("XObject", dictionary! {"Fm" => form_id});
            page_contents = b"/Fm Do\n".to_vec();
        }
        "image_used" | "image_unused_resource" | "image_unreferenced" => {
            let image = Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => indirect_space,
                },
                vec![0, 0, 0],
            );
            let image_id = document.add_object(image);
            if case != "image_unreferenced" {
                page_resources.set("XObject", dictionary! {"Im" => image_id});
            }
            page_contents = if case == "image_used" {
                b"/Im Do\n".to_vec()
            } else {
                Vec::new()
            };
        }
        "image_mask_ignores_color_space" => {
            let image_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 1,
                    "ImageMask" => true,
                    "ColorSpace" => indirect_space,
                },
                vec![0],
            ));
            page_resources.set("XObject", dictionary! {"Im" => image_id});
            page_contents = b"/Im Do\n".to_vec();
        }
        "image_smask_used" | "image_mask_image_used" | "image_alternate_used" => {
            let linked_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => indirect_space,
                },
                vec![0, 0, 0],
            ));
            let mut primary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "BitsPerComponent" => 8,
                "ColorSpace" => "DeviceRGB",
            };
            match case {
                "image_smask_used" => primary.set("SMask", linked_id),
                "image_mask_image_used" => primary.set("Mask", linked_id),
                _ => primary.set(
                    "Alternates",
                    vec![Object::Dictionary(dictionary! {
                        "Image" => linked_id,
                        "DefaultForPrinting" => true,
                    })],
                ),
            }
            let primary_id = document.add_object(Stream::new(primary, vec![0, 0, 0]));
            page_resources.set("XObject", dictionary! {"Im" => primary_id});
            page_contents = b"/Im Do\n".to_vec();
        }
        "inline_image_used" => {
            page_resources.set("ColorSpace", dictionary! {"CS1" => indirect_space});
            page_contents = b"q\nBI /W 1 /H 1 /BPC 8 /CS /CS1 ID \x00\x00\x00 EI\nQ\n".to_vec();
        }
        "shading_used" => {
            page_resources.set(
                "Shading",
                dictionary! {
                    "Sh1" => dictionary! {
                        "ShadingType" => 2,
                        "ColorSpace" => indirect_space,
                        "Coords" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                        "Function" => dictionary! {
                            "FunctionType" => 2,
                            "Domain" => vec![0.into(), 1.into()],
                            "C0" => vec![0.into(), 0.into(), 0.into()],
                            "C1" => vec![1.into(), 1.into(), 1.into()],
                            "N" => 1,
                        },
                        "Extend" => vec![Object::Boolean(true), Object::Boolean(true)],
                    },
                },
            );
            page_contents = b"/Sh1 sh\n".to_vec();
        }
        "indexed_base_used" => {
            page_resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![
                        Object::Name(b"Indexed".to_vec()),
                        indirect_space,
                        Object::Integer(1),
                        Object::String(vec![0; 6], lopdf::StringFormat::Hexadecimal),
                    ]),
                },
            );
        }
        "cyclic_indexed" => {
            let first_id = document.new_object_id();
            let second_id = document.new_object_id();
            document.objects.insert(
                first_id,
                Object::Array(vec![
                    Object::Name(b"Indexed".to_vec()),
                    Object::Reference(second_id),
                    Object::Integer(1),
                    Object::String(vec![0; 6], lopdf::StringFormat::Hexadecimal),
                ]),
            );
            document.objects.insert(
                second_id,
                Object::Array(vec![
                    Object::Name(b"Indexed".to_vec()),
                    Object::Reference(first_id),
                    Object::Integer(1),
                    Object::String(vec![0; 6], lopdf::StringFormat::Hexadecimal),
                ]),
            );
            page_resources.set(
                "ColorSpace",
                dictionary! {"CS1" => Object::Reference(first_id)},
            );
        }
        "deep_indexed" => {
            let mut nested = indirect_space;
            for _ in 0..8 {
                nested = Object::Array(vec![
                    Object::Name(b"Indexed".to_vec()),
                    nested,
                    Object::Integer(1),
                    Object::String(vec![0; 6], lopdf::StringFormat::Hexadecimal),
                ]);
            }
            page_resources.set("ColorSpace", dictionary! {"CS1" => nested});
        }
        _ => panic!("unknown ICCBased fixture case {case}"),
    }

    let content_id = document.add_object(Stream::new(Dictionary::new(), page_contents));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
    };
    if case != "inherited_resources" {
        page.set("Resources", page_resources.clone());
    }
    let page_id = document.add_object(page);
    let mut pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => 1,
    };
    if case == "inherited_resources" {
        pages.set("Resources", page_resources);
    }
    document.objects.insert(pages_id, Object::Dictionary(pages));
    let metadata_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        BASE_XMP.to_vec(),
    ));
    let intent_profile = profile_reference(&mut document, valid);
    let intent_id = document.add_object(output_intent_dictionary(
        Some(intent_profile),
        Some("GTS_PDFA1"),
    ));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => vec![Object::Reference(intent_id)],
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save ICCBased fixture");
    bytes
}

pub fn device_color_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let rgb_profile = icc_header(*b"mntr", *b"RGB ", 2, 1);
    let cmyk_profile = icc_header(*b"mntr", *b"CMYK", 2, 1);
    let mut resources = Dictionary::new();
    let mut contents = Vec::new();

    match case {
        "baseline" => {}
        "rgb_operator" | "rgb_with_cmyk_output" | "rgb_without_output" | "rgb_wrong_s" => {
            contents = b"0 0 0 rg\n0 0 0 RG\n".to_vec();
        }
        "cmyk_operator" | "cmyk_with_cmyk_output" | "cmyk_without_output" => {
            contents = b"0 0 0 0 k\n0 0 0 0 K\n".to_vec();
        }
        "gray_operator" | "gray_with_cmyk_output" | "gray_without_output" => {
            contents = b"0 g\n0 G\n".to_vec();
        }
        "explicit_rgb" => contents = b"/DeviceRGB cs\n/DeviceRGB CS\n".to_vec(),
        "resource_rgb" | "unused_resource_rgb" => {
            resources.set("ColorSpace", dictionary! {"CS1" => "DeviceRGB"});
            if case == "resource_rgb" {
                contents = b"/CS1 cs\n".to_vec();
            }
        }
        "default_rgb_override" => {
            let profile = profile_reference(&mut document, rgb_profile.clone());
            resources.set(
                "ColorSpace",
                dictionary! {
                    "DefaultRGB" => Object::Array(vec![
                        Object::Name(b"ICCBased".to_vec()),
                        profile,
                    ]),
                },
            );
            contents = b"0 0 0 rg\n0 0 0 RG\n".to_vec();
        }
        "form_rgb" => {
            let form_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                },
                b"0 0 0 rg\n".to_vec(),
            ));
            resources.set("XObject", dictionary! {"Fm" => form_id});
            contents = b"/Fm Do\n".to_vec();
        }
        "image_rgb" => {
            let image_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceRGB",
                },
                vec![0, 0, 0],
            ));
            resources.set("XObject", dictionary! {"Im" => image_id});
            contents = b"/Im Do\n".to_vec();
        }
        "inline_rgb" => {
            contents = b"q\nBI /W 1 /H 1 /BPC 8 /CS /RGB ID \x00\x00\x00 EI\nQ\n".to_vec();
        }
        "shading_rgb" => {
            resources.set(
                "Shading",
                dictionary! {
                    "Sh1" => dictionary! {
                        "ShadingType" => 2,
                        "ColorSpace" => "DeviceRGB",
                        "Coords" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                        "Function" => dictionary! {
                            "FunctionType" => 2,
                            "Domain" => vec![0.into(), 1.into()],
                            "C0" => vec![0.into(), 0.into(), 0.into()],
                            "C1" => vec![1.into(), 1.into(), 1.into()],
                            "N" => 1,
                        },
                    },
                },
            );
            contents = b"/Sh1 sh\n".to_vec();
        }
        "indexed_rgb" => {
            resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![
                        Object::Name(b"Indexed".to_vec()),
                        Object::Name(b"DeviceRGB".to_vec()),
                        Object::Integer(1),
                        Object::String(vec![0; 6], lopdf::StringFormat::Hexadecimal),
                    ]),
                },
            );
            contents = b"/CS1 cs\n".to_vec();
        }
        "separation_rgb" | "devicen_rgb" => {
            let tint_transform = dictionary! {
                "FunctionType" => 2,
                "Domain" => vec![0.into(), 1.into()],
                "C0" => vec![0.into(), 0.into(), 0.into()],
                "C1" => vec![1.into(), 1.into(), 1.into()],
                "N" => 1,
            };
            let color_space = if case == "separation_rgb" {
                Object::Array(vec![
                    Object::Name(b"Separation".to_vec()),
                    Object::Name(b"Spot".to_vec()),
                    Object::Name(b"DeviceRGB".to_vec()),
                    Object::Dictionary(tint_transform),
                ])
            } else {
                Object::Array(vec![
                    Object::Name(b"DeviceN".to_vec()),
                    Object::Array(vec![Object::Name(b"Spot".to_vec())]),
                    Object::Name(b"DeviceRGB".to_vec()),
                    Object::Dictionary(tint_transform),
                ])
            };
            resources.set("ColorSpace", dictionary! {"CS1" => color_space});
            contents = b"/CS1 cs\n".to_vec();
        }
        "pattern_rgb" => {
            let pattern_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 2,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "XStep" => 1,
                    "YStep" => 1,
                    "Resources" => Dictionary::new(),
                },
                b"0 0 1 1 re f\n".to_vec(),
            ));
            resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![
                        Object::Name(b"Pattern".to_vec()),
                        Object::Name(b"DeviceRGB".to_vec()),
                    ]),
                },
            );
            resources.set("Pattern", dictionary! {"P1" => pattern_id});
            contents = b"/CS1 cs\n1 0 0 /P1 scn\n0 0 10 10 re f\n".to_vec();
        }
        _ => panic!("unknown device-colour fixture case {case}"),
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), contents));
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources,
        "Contents" => contents_id,
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
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
    };
    if !matches!(
        case,
        "rgb_without_output" | "cmyk_without_output" | "gray_without_output"
    ) {
        let output_bytes = if matches!(
            case,
            "rgb_with_cmyk_output"
                | "cmyk_with_cmyk_output"
                | "gray_with_cmyk_output"
                | "unused_resource_rgb"
                | "default_rgb_override"
                | "explicit_rgb"
                | "resource_rgb"
                | "form_rgb"
                | "image_rgb"
                | "inline_rgb"
                | "shading_rgb"
                | "indexed_rgb"
                | "separation_rgb"
                | "devicen_rgb"
                | "pattern_rgb"
        ) {
            cmyk_profile
        } else {
            rgb_profile
        };
        let profile = profile_reference(&mut document, output_bytes);
        let subtype = if case == "rgb_wrong_s" {
            "GTS_PDFX"
        } else {
            "GTS_PDFA1"
        };
        let intent_id = document.add_object(output_intent_dictionary(Some(profile), Some(subtype)));
        catalog.set("OutputIntents", vec![Object::Reference(intent_id)]);
    }
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save device-colour fixture");
    bytes
}

pub fn xobject_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let mut resources = Dictionary::new();
    let mut contents = b"/XO Do\n".to_vec();
    let mut include_resource = true;
    let explicit_mask_id = (case == "explicit_mask_bpc_8").then(|| {
        document.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "BitsPerComponent" => 8,
            },
            vec![0],
        ))
    });

    let xobject = match case {
        "baseline" | "image_interpolate_false" | "image_bpc_missing" => {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "ColorSpace" => "DeviceRGB",
            };
            if case == "baseline" {
                dictionary.set("BitsPerComponent", 8);
            } else if case == "image_interpolate_false" {
                dictionary.set("BitsPerComponent", 8);
                dictionary.set("Interpolate", false);
            }
            Stream::new(dictionary, vec![0, 0, 0])
        }
        "image_alternates"
        | "image_opi"
        | "image_interpolate_true"
        | "image_bpc_16"
        | "unused_resource_invalid_image"
        | "unreferenced_invalid_image"
        | "two_invalid_images" => {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "BitsPerComponent" => 8,
                "ColorSpace" => "DeviceRGB",
            };
            match case {
                "image_alternates" => dictionary.set("Alternates", Vec::<Object>::new()),
                "image_opi" => dictionary.set("OPI", Dictionary::new()),
                "image_interpolate_true" => dictionary.set("Interpolate", true),
                "image_bpc_16"
                | "unused_resource_invalid_image"
                | "unreferenced_invalid_image"
                | "two_invalid_images" => dictionary.set("BitsPerComponent", 16),
                _ => unreachable!(),
            }
            if case == "unused_resource_invalid_image" {
                contents.clear();
            } else if case == "unreferenced_invalid_image" {
                contents.clear();
                include_resource = false;
            }
            Stream::new(dictionary, vec![0, 0, 0])
        }
        "mask_bpc_8" | "mask_bpc_missing" => {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "ImageMask" => true,
            };
            if case == "mask_bpc_8" {
                dictionary.set("BitsPerComponent", 8);
            }
            Stream::new(dictionary, vec![0])
        }
        "explicit_mask_bpc_8" => Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "BitsPerComponent" => 8,
                "ColorSpace" => "DeviceRGB",
                "Mask" => explicit_mask_id.expect("explicit mask object"),
            },
            vec![0, 0, 0],
        ),
        "form_opi" | "form_ps_key" | "form_ps_null" | "form_subtype2_ps" | "form_ref" => {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Form",
                "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            };
            match case {
                "form_opi" => dictionary.set("OPI", Dictionary::new()),
                "form_ps_key" => {
                    let postscript =
                        document.add_object(Stream::new(Dictionary::new(), b"%!PS\n".to_vec()));
                    dictionary.set("PS", postscript);
                }
                "form_ps_null" => dictionary.set("PS", Object::Null),
                "form_subtype2_ps" => dictionary.set("Subtype2", "PS"),
                "form_ref" => dictionary.set("Ref", Dictionary::new()),
                _ => unreachable!(),
            }
            Stream::new(dictionary, Vec::new())
        }
        "postscript_xobject" => Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "PS",
            },
            b"%!PS\n".to_vec(),
        ),
        _ => panic!("unknown XObject fixture case {case}"),
    };
    let xobject_id = document.add_object(xobject);
    if include_resource {
        let mut xobjects = dictionary! {"XO" => xobject_id};
        if case == "two_invalid_images" {
            let second_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 16,
                    "ColorSpace" => "DeviceRGB",
                },
                vec![0, 0, 0],
            ));
            xobjects.set("XO2", second_id);
            contents = b"/XO Do\n/XO2 Do\n".to_vec();
        }
        resources.set("XObject", xobjects);
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), contents));
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources,
        "Contents" => contents_id,
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
    let output_profile = profile_reference(&mut document, icc_header(*b"mntr", *b"RGB ", 2, 1));
    let intent_id = document.add_object(output_intent_dictionary(
        Some(output_profile),
        Some("GTS_PDFA1"),
    ));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => vec![Object::Reference(intent_id)],
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save XObject fixture");
    bytes
}

pub fn graphics_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let mut resources = Dictionary::new();
    let mut contents = Vec::new();
    let mut page_group = None;

    match case {
        "baseline" => {}
        "extgstate_tr"
        | "extgstate_tr2_default"
        | "extgstate_tr2_other"
        | "extgstate_ri_invalid"
        | "unused_extgstate_tr"
        | "unreferenced_extgstate_tr"
        | "extgstate_smask_none"
        | "extgstate_smask_other"
        | "extgstate_smask_dictionary"
        | "extgstate_bm_normal"
        | "extgstate_bm_compatible"
        | "extgstate_bm_multiply"
        | "extgstate_stroke_alpha_one"
        | "extgstate_stroke_alpha_zero"
        | "extgstate_fill_alpha_one"
        | "extgstate_fill_alpha_zero"
        | "unused_extgstate_transparency" => {
            let mut state = Dictionary::new();
            match case {
                "extgstate_tr" | "unused_extgstate_tr" | "unreferenced_extgstate_tr" => {
                    state.set("TR", "Identity");
                }
                "extgstate_tr2_default" => state.set("TR2", "Default"),
                "extgstate_tr2_other" => state.set("TR2", "Identity"),
                "extgstate_ri_invalid" => state.set("RI", "MaiIntent"),
                "extgstate_smask_none" => state.set("SMask", "None"),
                "extgstate_smask_other" | "unused_extgstate_transparency" => {
                    state.set("SMask", "Alpha")
                }
                "extgstate_smask_dictionary" => state.set("SMask", Dictionary::new()),
                "extgstate_bm_normal" => state.set("BM", "Normal"),
                "extgstate_bm_compatible" => state.set("BM", "Compatible"),
                "extgstate_bm_multiply" => state.set("BM", "Multiply"),
                "extgstate_stroke_alpha_one" => state.set("CA", 1),
                "extgstate_stroke_alpha_zero" => state.set("CA", 0),
                "extgstate_fill_alpha_one" => state.set("ca", 1),
                "extgstate_fill_alpha_zero" => state.set("ca", 0),
                _ => unreachable!(),
            }
            let state_id = document.add_object(state);
            if case != "unreferenced_extgstate_tr" {
                resources.set("ExtGState", dictionary! {"GS1" => state_id});
            }
            if !matches!(
                case,
                "unused_extgstate_tr"
                    | "unreferenced_extgstate_tr"
                    | "unused_extgstate_transparency"
            ) {
                contents = b"/GS1 gs\n".to_vec();
            }
        }
        "ri_standard" => {
            contents = b"/RelativeColorimetric ri\n/AbsoluteColorimetric ri\n/Perceptual ri\n/Saturation ri\n".to_vec();
        }
        "ri_invalid" => contents = b"/MaiIntent ri\n".to_vec(),
        "image_intent_valid" | "image_intent_invalid" => {
            let intent = if case == "image_intent_valid" {
                "Perceptual"
            } else {
                "MaiIntent"
            };
            let image_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceRGB",
                    "Intent" => intent,
                },
                vec![0, 0, 0],
            ));
            resources.set("XObject", dictionary! {"Im" => image_id});
            contents = b"/Im Do\n".to_vec();
        }
        "undefined_operator" => contents = b"1 2 MaiUnknown\n".to_vec(),
        "undefined_in_bx" => contents = b"BX\nMaiUnknown\nEX\n".to_vec(),
        "known_operators" => contents = b"q\n0 0 m\n1 1 l\nS\nQ\n".to_vec(),
        "undefined_form" | "unused_form_undefined" => {
            let form_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                },
                b"MaiUnknown\n".to_vec(),
            ));
            resources.set("XObject", dictionary! {"Fm" => form_id});
            if case == "undefined_form" {
                contents = b"/Fm Do\n".to_vec();
            }
        }
        "xobject_smask" | "unused_xobject_smask" => {
            let soft_mask = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceGray",
                },
                vec![0],
            ));
            let image = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceRGB",
                    "SMask" => soft_mask,
                },
                vec![0, 0, 0],
            ));
            resources.set("XObject", dictionary! {"Im" => image});
            if case == "xobject_smask" {
                contents = b"/Im Do\n".to_vec();
            }
        }
        "page_transparency_group" | "page_nontransparency_group" => {
            page_group = Some(dictionary! {
                "Type" => "Group",
                "S" => if case == "page_transparency_group" {
                    Object::Name(b"Transparency".to_vec())
                } else {
                    Object::Name(b"MaiGroup".to_vec())
                },
            });
        }
        "form_transparency_group" | "unused_form_transparency_group" => {
            let form = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Group" => dictionary! {
                        "Type" => "Group",
                        "S" => "Transparency",
                    },
                },
                Vec::new(),
            ));
            resources.set("XObject", dictionary! {"Fm" => form});
            if case == "form_transparency_group" {
                contents = b"/Fm Do\n".to_vec();
            }
        }
        _ => panic!("unknown graphics fixture case {case}"),
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), contents));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources,
        "Contents" => contents_id,
    };
    if let Some(group) = page_group {
        page.set("Group", group);
    }
    let page_id = document.add_object(page);
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
    let output_profile = profile_reference(&mut document, icc_header(*b"mntr", *b"RGB ", 2, 1));
    let intent_id = document.add_object(output_intent_dictionary(
        Some(output_profile),
        Some("GTS_PDFA1"),
    ));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => vec![Object::Reference(intent_id)],
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save graphics fixture");
    bytes
}

pub fn annotation_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let appearance_stream = document.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        },
        Vec::new(),
    ));
    let mut annotation = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Text",
        "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        "F" => 4,
    };
    let mut include_annotation = true;
    let mut direct_annotation = false;
    let mut output_color_space = Some(*b"RGB ");

    match case {
        "baseline" | "opacity_absent" | "appearance_absent" => {}
        "subtype_widget" => annotation.set("Subtype", "Widget"),
        "subtype_trapnet" => annotation.set("Subtype", "TrapNet"),
        "subtype_file_attachment" => annotation.set("Subtype", "FileAttachment"),
        "subtype_unknown" | "unreferenced_invalid_annotation" => {
            annotation.set("Subtype", "MaiAnnot")
        }
        "subtype_missing" => {
            annotation.remove(b"Subtype");
        }
        "direct_invalid_annotation" => {
            annotation.set("Subtype", "MaiAnnot");
            direct_annotation = true;
        }
        "opacity_one" => annotation.set("CA", 1),
        "opacity_zero" => annotation.set("CA", 0),
        "opacity_wrong_type" => annotation.set("CA", "Opaque"),
        "flags_missing" => {
            annotation.remove(b"F");
        }
        "flags_not_printable" => annotation.set("F", 0),
        "flags_invisible" => annotation.set("F", 5),
        "flags_hidden" => annotation.set("F", 6),
        "flags_no_view" => annotation.set("F", 36),
        "color_c_rgb" => annotation.set("C", vec![1.into(), 0.into(), 0.into()]),
        "color_ic_rgb" => annotation.set("IC", vec![1.into(), 0.into(), 0.into()]),
        "color_c_cmyk" => {
            annotation.set("C", vec![1.into(), 0.into(), 0.into()]);
            output_color_space = Some(*b"CMYK");
        }
        "color_ic_without_output" => {
            annotation.set("IC", vec![1.into(), 0.into(), 0.into()]);
            output_color_space = None;
        }
        "no_color_cmyk" => output_color_space = Some(*b"CMYK"),
        "appearance_n_stream" => annotation.set("AP", dictionary! {"N" => appearance_stream}),
        "appearance_n_dictionary" => annotation.set(
            "AP",
            dictionary! {
                "N" => dictionary! {
                    "On" => appearance_stream,
                },
            },
        ),
        "appearance_n_and_r" => annotation.set(
            "AP",
            dictionary! {
                "N" => appearance_stream,
                "R" => appearance_stream,
            },
        ),
        "appearance_empty" => annotation.set("AP", Dictionary::new()),
        "appearance_wrong_type" => annotation.set("AP", 42),
        "widget_button_dictionary" | "widget_button_empty_dictionary" => {
            annotation.set("Subtype", "Widget");
            annotation.set("FT", "Btn");
            annotation.set(
                "AP",
                dictionary! {
                    "N" => if case == "widget_button_dictionary" {
                        Object::Dictionary(dictionary! {"Yes" => appearance_stream})
                    } else {
                        Object::Dictionary(Dictionary::new())
                    },
                },
            );
        }
        "widget_button_stream" => {
            annotation.set("Subtype", "Widget");
            annotation.set("FT", "Btn");
            annotation.set("AP", dictionary! {"N" => appearance_stream});
        }
        "widget_text_stream" => {
            annotation.set("Subtype", "Widget");
            annotation.set("FT", "Tx");
            annotation.set("AP", dictionary! {"N" => appearance_stream});
        }
        "widget_inherited_button_dictionary" => {
            let parent = document.add_object(dictionary! {
                "FT" => "Btn",
            });
            annotation.set("Subtype", "Widget");
            annotation.set("Parent", parent);
            annotation.set(
                "AP",
                dictionary! {
                    "N" => dictionary! {"Yes" => appearance_stream},
                },
            );
        }
        _ => panic!("unknown annotation fixture case {case}"),
    }

    if case == "unreferenced_invalid_annotation" {
        include_annotation = false;
    }
    let annotation_object = if direct_annotation {
        Object::Dictionary(annotation)
    } else {
        Object::Reference(document.add_object(annotation))
    };
    let contents_id = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => Dictionary::new(),
        "Contents" => contents_id,
    };
    if include_annotation {
        page.set("Annots", vec![annotation_object]);
    }
    let page_id = document.add_object(page);
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
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
    };
    if let Some(color_space) = output_color_space {
        let profile = profile_reference(&mut document, icc_header(*b"mntr", color_space, 2, 1));
        let intent =
            document.add_object(output_intent_dictionary(Some(profile), Some("GTS_PDFA1")));
        catalog.set("OutputIntents", vec![Object::Reference(intent)]);
    }
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save annotation fixture");
    bytes
}

pub fn action_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let widget_appearance = document.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        },
        Vec::new(),
    ));
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => Dictionary::new(),
    };
    let mut annotations = Vec::new();
    let mut fields = Vec::new();

    match case {
        "baseline" => {}
        "allowed_goto" => catalog.set("OpenAction", action("GoTo")),
        "allowed_gotor" => catalog.set("OpenAction", action("GoToR")),
        "allowed_thread" => catalog.set("OpenAction", action("Thread")),
        "allowed_uri" => catalog.set("OpenAction", action("URI")),
        "allowed_named" => {
            let mut named = action_dictionary("Named");
            named.set("N", "NextPage");
            catalog.set("OpenAction", named);
        }
        "allowed_submit_form" => catalog.set("OpenAction", action("SubmitForm")),
        "open_javascript" | "open_javascript_indirect" => {
            let javascript = action_dictionary("JavaScript");
            if case == "open_javascript_indirect" {
                catalog.set("OpenAction", document.add_object(javascript));
            } else {
                catalog.set("OpenAction", javascript);
            }
        }
        "open_missing_subtype" => catalog.set("OpenAction", Dictionary::new()),
        "open_wrong_subtype_type" => {
            catalog.set(
                "OpenAction",
                dictionary! {"S" => Object::string_literal("GoTo")},
            );
        }
        "open_destination_array" => {
            catalog.set(
                "OpenAction",
                vec![Object::Reference(pages_id), Object::Name(b"Fit".to_vec())],
            );
        }
        "unreferenced_javascript" => {
            document.add_object(action_dictionary("JavaScript"));
        }
        "page_additional_action" => {
            page.set("AA", dictionary! {"O" => action("JavaScript")});
        }
        "page_unknown_additional_action" => {
            page.set("AA", dictionary! {"X" => action("JavaScript")});
        }
        "annotation_action" => {
            let mut annotation = valid_annotation("Text");
            annotation.set("A", action("JavaScript"));
            annotations.push(Object::Reference(document.add_object(annotation)));
        }
        "annotation_additional_action" => {
            let mut annotation = valid_annotation("Text");
            annotation.set("AA", dictionary! {"E" => action("JavaScript")});
            annotations.push(Object::Reference(document.add_object(annotation)));
        }
        "annotation_unknown_additional_action" => {
            let mut annotation = valid_annotation("Text");
            annotation.set("AA", dictionary! {"K" => action("JavaScript")});
            annotations.push(Object::Reference(document.add_object(annotation)));
        }
        "outline_action" => {
            let outlines_id = document.new_object_id();
            let outline_id = document.add_object(dictionary! {
                "Title" => Object::string_literal("Mai outline"),
                "Parent" => outlines_id,
                "A" => action("JavaScript"),
            });
            document.objects.insert(
                outlines_id,
                Object::Dictionary(dictionary! {
                    "Type" => "Outlines",
                    "First" => outline_id,
                    "Last" => outline_id,
                    "Count" => 1,
                }),
            );
            catalog.set("Outlines", outlines_id);
        }
        "next_action" => {
            let mut first = action_dictionary("GoTo");
            first.set("Next", action("JavaScript"));
            catalog.set("OpenAction", first);
        }
        "next_action_array" => {
            let mut first = action_dictionary("GoTo");
            first.set("Next", vec![action("URI"), action("JavaScript"), 42.into()]);
            catalog.set("OpenAction", first);
        }
        "named_next_page" => {
            let mut named = action_dictionary("Named");
            named.set("N", "NextPage");
            catalog.set("OpenAction", named);
        }
        "named_prev_page" => {
            let mut named = action_dictionary("Named");
            named.set("N", "PrevPage");
            catalog.set("OpenAction", named);
        }
        "named_first_page" => {
            let mut named = action_dictionary("Named");
            named.set("N", "FirstPage");
            catalog.set("OpenAction", named);
        }
        "named_last_page" => {
            let mut named = action_dictionary("Named");
            named.set("N", "LastPage");
            catalog.set("OpenAction", named);
        }
        "named_forbidden" => {
            let mut named = action_dictionary("Named");
            named.set("N", "Print");
            catalog.set("OpenAction", named);
        }
        "named_missing" => catalog.set("OpenAction", action("Named")),
        "named_wrong_type" => {
            let mut named = action_dictionary("Named");
            named.set("N", Object::string_literal("NextPage"));
            catalog.set("OpenAction", named);
        }
        "non_named_with_forbidden_n" => {
            let mut uri = action_dictionary("URI");
            uri.set("N", "Print");
            catalog.set("OpenAction", uri);
        }
        "widget_action" | "widget_action_wrong_type" => {
            let mut widget = valid_annotation("Widget");
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set(
                "A",
                if case == "widget_action" {
                    action("URI")
                } else {
                    42.into()
                },
            );
            annotations.push(Object::Reference(document.add_object(widget)));
        }
        "widget_additional_actions" => {
            let mut widget = valid_annotation("Widget");
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set("AA", Dictionary::new());
            annotations.push(Object::Reference(document.add_object(widget)));
        }
        "widget_additional_javascript" => {
            let mut widget = valid_annotation("Widget");
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set("AA", dictionary! {"E" => action("JavaScript")});
            annotations.push(Object::Reference(document.add_object(widget)));
        }
        "text_additional_actions" => {
            let mut annotation = valid_annotation("Text");
            annotation.set("AA", Dictionary::new());
            annotations.push(Object::Reference(document.add_object(annotation)));
        }
        "field_additional_actions" | "top_field_without_t" => {
            let mut field = dictionary! {
                "T" => Object::string_literal("field"),
                "FT" => "Tx",
                "AA" => Dictionary::new(),
            };
            if case == "top_field_without_t" {
                field.remove(b"T");
            }
            fields.push(Object::Reference(document.add_object(field)));
        }
        "field_additional_javascript" => {
            fields.push(Object::Reference(document.add_object(dictionary! {
                "T" => Object::string_literal("field"),
                "FT" => "Tx",
                "AA" => dictionary! {"K" => action("JavaScript")},
            })));
        }
        "child_field_additional_actions" | "child_without_t" => {
            let mut child = dictionary! {
                "T" => Object::string_literal("child"),
                "AA" => Dictionary::new(),
            };
            if case == "child_without_t" {
                child.remove(b"T");
            }
            let child_id = document.add_object(child);
            fields.push(Object::Reference(document.add_object(dictionary! {
                "T" => Object::string_literal("parent"),
                "FT" => "Tx",
                "Kids" => vec![Object::Reference(child_id)],
            })));
        }
        "unreferenced_field_additional_actions" => {
            document.add_object(dictionary! {
                "T" => Object::string_literal("unreferenced"),
                "AA" => Dictionary::new(),
            });
        }
        "combined_widget_field_actions" => {
            let mut widget = valid_annotation("Widget");
            widget.set("T", Object::string_literal("combined"));
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set("A", action("URI"));
            widget.set("AA", Dictionary::new());
            let widget_id = document.add_object(widget);
            annotations.push(Object::Reference(widget_id));
            fields.push(Object::Reference(widget_id));
        }
        "catalog_additional_actions" => catalog.set("AA", Dictionary::new()),
        "catalog_additional_javascript" => {
            catalog.set("AA", dictionary! {"WC" => action("JavaScript")});
        }
        "catalog_unknown_additional_action" => {
            catalog.set("AA", dictionary! {"X" => action("JavaScript")});
        }
        "catalog_additional_actions_wrong_type" => catalog.set("AA", 42),
        _ => panic!("unknown action fixture case {case}"),
    }

    if !fields.is_empty() {
        catalog.set("AcroForm", dictionary! {"Fields" => fields});
    }
    if !annotations.is_empty() {
        page.set("Annots", annotations);
    }
    let contents_id = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
    page.set("Contents", contents_id);
    let page_id = document.add_object(page);
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
    catalog.set("Metadata", metadata_id);
    let profile = profile_reference(&mut document, icc_header(*b"mntr", *b"RGB ", 2, 1));
    let intent = document.add_object(output_intent_dictionary(Some(profile), Some("GTS_PDFA1")));
    catalog.set("OutputIntents", vec![Object::Reference(intent)]);
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save action fixture");
    bytes
}

pub fn form_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let appearance_stream = document.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        },
        Vec::new(),
    ));
    let mut widget = valid_annotation("Widget");
    widget.set("FT", "Tx");
    widget.set("T", Object::string_literal("field"));
    widget.set("AP", dictionary! {"N" => appearance_stream});
    let mut include_on_page = true;
    let mut include_as_field = true;
    let mut direct_widget = false;
    let mut include_acro_form = true;
    let mut acro_form = dictionary! {
        "NeedAppearances" => false,
    };
    let mut acro_form_override = None;

    match case {
        "baseline" => {}
        "no_acroform" => include_acro_form = false,
        "need_appearances_absent" => {
            acro_form.remove(b"NeedAppearances");
        }
        "need_appearances_true" => acro_form.set("NeedAppearances", true),
        "need_appearances_false_indirect" => {
            acro_form.set(
                "NeedAppearances",
                document.add_object(Object::Boolean(false)),
            );
        }
        "need_appearances_true_indirect" => {
            acro_form.set(
                "NeedAppearances",
                document.add_object(Object::Boolean(true)),
            );
        }
        "need_appearances_wrong_type" => acro_form.set("NeedAppearances", 1),
        "need_appearances_null" => acro_form.set("NeedAppearances", Object::Null),
        "acroform_wrong_type" => acro_form_override = Some(42.into()),
        "acroform_stream_true" => {
            acro_form_override = Some(Object::Stream(Stream::new(
                dictionary! {"NeedAppearances" => true},
                Vec::new(),
            )));
        }
        "widget_missing_ap" => {
            widget.remove(b"AP");
        }
        "widget_empty_ap" => widget.set("AP", Dictionary::new()),
        "widget_wrong_type_ap" => widget.set("AP", 42),
        "widget_stream_ap" => widget.set("AP", appearance_stream),
        "widget_indirect_ap" => {
            let appearance = document.add_object(dictionary! {"N" => appearance_stream});
            widget.set("AP", appearance);
        }
        "non_widget_missing_ap" => {
            widget.set("Subtype", "Text");
            widget.remove(b"AP");
            include_as_field = false;
        }
        "field_only_widget_missing_ap" => {
            widget.remove(b"AP");
            include_on_page = false;
        }
        "direct_widget_missing_ap" => {
            widget.remove(b"AP");
            direct_widget = true;
            include_as_field = false;
        }
        "widget_parent_ap_only" => {
            let parent = document.add_object(dictionary! {
                "FT" => "Tx",
                "AP" => dictionary! {"N" => appearance_stream},
            });
            widget.remove(b"AP");
            widget.set("Parent", parent);
            include_as_field = false;
        }
        "unreferenced_widget_missing_ap" => {
            widget.remove(b"AP");
            include_on_page = false;
            include_as_field = false;
        }
        _ => panic!("unknown form fixture case {case}"),
    }

    let widget_value = if direct_widget {
        Object::Dictionary(widget)
    } else {
        Object::Reference(document.add_object(widget))
    };
    if include_as_field {
        acro_form.set("Fields", vec![widget_value.clone()]);
    } else {
        acro_form.set("Fields", Vec::<Object>::new());
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => Dictionary::new(),
        "Contents" => contents_id,
    };
    if include_on_page {
        page.set("Annots", vec![widget_value]);
    }
    let page_id = document.add_object(page);
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
    let profile = profile_reference(&mut document, icc_header(*b"mntr", *b"RGB ", 2, 1));
    let intent = document.add_object(output_intent_dictionary(Some(profile), Some("GTS_PDFA1")));
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => vec![Object::Reference(intent)],
    };
    if include_acro_form {
        let acro_form = acro_form_override.unwrap_or_else(|| Object::Dictionary(acro_form));
        catalog.set("AcroForm", document.add_object(acro_form));
    }
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save form fixture");
    bytes
}

pub fn document_feature_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let contents_id = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => Dictionary::new(),
        "Contents" => contents_id,
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
    let profile = profile_reference(&mut document, icc_header(*b"mntr", *b"RGB ", 2, 1));
    let intent = document.add_object(output_intent_dictionary(Some(profile), Some("GTS_PDFA1")));
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => vec![Object::Reference(intent)],
    };

    match case {
        "baseline" => {}
        "names_empty" => catalog.set("Names", Dictionary::new()),
        "names_embedded_files_dictionary" => {
            catalog.set("Names", dictionary! {"EmbeddedFiles" => Dictionary::new()});
        }
        "names_embedded_files_wrong_type" => {
            catalog.set("Names", dictionary! {"EmbeddedFiles" => 42});
        }
        "names_embedded_files_null" => {
            catalog.set("Names", dictionary! {"EmbeddedFiles" => Object::Null});
        }
        "names_embedded_files_indirect_null" => {
            let null = document.add_object(Object::Null);
            catalog.set("Names", dictionary! {"EmbeddedFiles" => null});
        }
        "names_stream_embedded_files" => {
            let names =
                document.add_object(Stream::new(dictionary! {"EmbeddedFiles" => 42}, Vec::new()));
            catalog.set("Names", names);
        }
        "names_wrong_type" => catalog.set("Names", 42),
        "names_indirect_dictionary" => {
            let names = document.add_object(dictionary! {"EmbeddedFiles" => 42});
            catalog.set("Names", names);
        }
        "unreferenced_names_embedded_files" => {
            document.add_object(dictionary! {"EmbeddedFiles" => 42});
        }
        "file_spec_without_ef" => {
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Dictionary(Dictionary::new()),
                        ],
                    },
                },
            );
        }
        "file_spec_with_ef" => {
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Dictionary(dictionary! {"EF" => Dictionary::new()}),
                        ],
                    },
                },
            );
        }
        "file_spec_indirect_with_ef" => {
            let file_spec = document.add_object(dictionary! {"EF" => Dictionary::new()});
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Reference(file_spec),
                        ],
                    },
                },
            );
        }
        "file_spec_stream_with_ef" => {
            let file_spec = document.add_object(Stream::new(
                dictionary! {"EF" => Dictionary::new()},
                Vec::new(),
            ));
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Reference(file_spec),
                        ],
                    },
                },
            );
        }
        "file_spec_scalar" => {
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![Object::string_literal("file"), Object::Integer(42)],
                    },
                },
            );
        }
        "embedded_files_kids_with_ef" => {
            let child = document.add_object(dictionary! {
                "Names" => vec![
                    Object::string_literal("file"),
                    Object::Dictionary(dictionary! {"EF" => Dictionary::new()}),
                ],
            });
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Kids" => vec![Object::Reference(child)],
                    },
                },
            );
        }
        "stream_f" => {
            document.add_object(Stream::new(dictionary! {"F" => "external"}, Vec::new()));
        }
        "stream_ffilter" => {
            document.add_object(Stream::new(
                dictionary! {"FFilter" => "FlateDecode"},
                Vec::new(),
            ));
        }
        "stream_fdecodeparms" => {
            document.add_object(Stream::new(dictionary! {"FDecodeParms" => 42}, Vec::new()));
        }
        "stream_external_null" => {
            document.add_object(Stream::new(dictionary! {"F" => Object::Null}, Vec::new()));
        }
        "stream_lzwdecode" => {
            document.add_object(Stream::new(
                dictionary! {"Filter" => "LZWDecode"},
                Vec::new(),
            ));
        }
        "stream_lzwdecode_array" => {
            document.add_object(Stream::new(
                dictionary! {"Filter" => vec!["FlateDecode".into(), "LZWDecode".into()]},
                Vec::new(),
            ));
        }
        "stream_lzwdecode_indirect" => {
            let filter = document.add_object("LZWDecode");
            document.add_object(Stream::new(dictionary! {"Filter" => filter}, Vec::new()));
        }
        "stream_lzw_short_name" => {
            document.add_object(Stream::new(dictionary! {"Filter" => "LZW"}, Vec::new()));
        }
        "ocproperties_dictionary" => catalog.set("OCProperties", Dictionary::new()),
        "ocproperties_wrong_type" => catalog.set("OCProperties", 42),
        "ocproperties_null" => catalog.set("OCProperties", Object::Null),
        "ocproperties_indirect_null" => {
            let null = document.add_object(Object::Null);
            catalog.set("OCProperties", null);
        }
        "ocproperties_stream" => {
            let value = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
            catalog.set("OCProperties", value);
        }
        "unreferenced_catalog_ocproperties" => {
            document.add_object(dictionary! {
                "Type" => "Catalog",
                "OCProperties" => Dictionary::new(),
            });
        }
        _ => panic!("unknown document-feature fixture case {case}"),
    }

    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save document-feature fixture");
    bytes
}

fn action(subtype: &str) -> Object {
    Object::Dictionary(action_dictionary(subtype))
}

fn action_dictionary(subtype: &str) -> Dictionary {
    dictionary! {
        "Type" => "Action",
        "S" => subtype,
    }
}

fn valid_annotation(subtype: &str) -> Dictionary {
    dictionary! {
        "Type" => "Annot",
        "Subtype" => subtype,
        "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        "F" => 4,
    }
}

pub fn font_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let embedded = !matches!(
        case,
        "unembedded_visible"
            | "unembedded_invisible"
            | "mixed_rendering_modes"
            | "mixed_visible_first"
            | "unused_resource"
            | "selected_not_shown"
            | "direct_font"
            | "form_unembedded"
            | "nested_form_unembedded"
            | "inherited_resources"
            | "repeated_aliases"
            | "two_unembedded_fonts"
            | "type0_unembedded_descendant"
            | "missing_descriptor"
            | "malformed_descriptor"
            | "graphics_state_visible"
            | "graphics_state_invisible"
            | "cyclic_form"
            | "large_content"
    );
    let mut descriptor = font_descriptor(&mut document, embedded);
    if case.starts_with("tt_symbolic_") {
        descriptor.set("Flags", 4);
    }
    if case == "tt_symbolic_two_cmaps" {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(
                Dictionary::new(),
                minimal_truetype_with_cmap_count(2),
            )),
        );
    }
    if case == "direct_font_file" {
        descriptor.set(
            "FontFile2",
            Object::Stream(Stream::new(Dictionary::new(), minimal_truetype())),
        );
    } else if case == "malformed_font_program" {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(Dictionary::new(), b"not a font".to_vec())),
        );
    } else if case == "malformed_font_file" {
        descriptor.set("FontFile2", 42);
    } else if case == "missing_font_file_object" {
        descriptor.set("FontFile2", Object::Reference((999_999, 0)));
    } else if case == "font_file_subtype_invalid" {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(
                dictionary! {
                    "Subtype" => "OpenType",
                },
                minimal_truetype(),
            )),
        );
    }
    let descriptor_object = if matches!(case, "direct_descriptor" | "direct_font_file") {
        Object::Dictionary(descriptor)
    } else {
        Object::Reference(document.add_object(descriptor))
    };
    let mut font = dictionary! {
       "Type" => "Font",
       "Subtype" => "TrueType",
       "BaseFont" => "MaiTestFont",
       "Encoding" => "WinAnsiEncoding",
       "FirstChar" => 32,
       "LastChar" => 32,
       "Widths" => vec![500.into()],
       "FontDescriptor" => descriptor_object.clone(),
    };
    if case == "missing_descriptor" {
        font.remove(b"FontDescriptor");
    } else if case == "malformed_descriptor" {
        font.set("FontDescriptor", 42);
    }
    if case == "type3_visible" {
        font = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type3",
            "FontBBox" => vec![0.into(), 0.into(), 500.into(), 700.into()],
            "FontMatrix" => vec![0.001.into(), 0.into(), 0.into(), 0.001.into(), 0.into(), 0.into()],
            "CharProcs" => Dictionary::new(),
            "Encoding" => dictionary! {
                "Type" => "Encoding",
                "Differences" => vec![32.into(), Object::Name(b"space".to_vec())],
            },
            "FirstChar" => 32,
            "LastChar" => 32,
            "Widths" => vec![500.into()],
        };
    } else if matches!(
        case,
        "type0_unembedded_descendant" | "type0_embedded_descendant"
    ) || case.starts_with("composite_")
    {
        let mut descendant_dictionary = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => "MaiTestFont",
            "CIDSystemInfo" => dictionary! {
                "Registry" => Object::string_literal("Adobe"),
                "Ordering" => Object::string_literal("Identity"),
                "Supplement" => 0,
            },
            "FontDescriptor" => descriptor_object,
            "DW" => 500,
            "CIDToGIDMap" => "Identity",
        };
        match case {
            "composite_cidmap_missing" => {
                descendant_dictionary.remove(b"CIDToGIDMap");
            }
            "composite_cidmap_invalid_name" => {
                descendant_dictionary.set("CIDToGIDMap", "NotIdentity");
            }
            "composite_cidmap_stream" => {
                let map = document.add_object(Stream::new(Dictionary::new(), vec![0, 0]));
                descendant_dictionary.set("CIDToGIDMap", map);
            }
            _ => {}
        }
        let descendant = document.add_object(descendant_dictionary);
        let encoding = match case {
            "composite_identity_v" => Object::Name(b"Identity-V".to_vec()),
            "composite_named_cmap" => Object::Name(b"UniJIS-UCS2-H".to_vec()),
            "composite_cmap_matching"
            | "composite_cmap_mismatch_system"
            | "composite_cmap_wmode_match"
            | "composite_cmap_wmode_mismatch" => {
                let cmap_ordering = if case == "composite_cmap_mismatch_system" {
                    "Japan1"
                } else {
                    "Identity"
                };
                let dictionary_wmode = i64::from(case == "composite_cmap_wmode_match")
                    + i64::from(case == "composite_cmap_wmode_mismatch");
                let content_wmode = i64::from(case == "composite_cmap_wmode_match");
                Object::Reference(document.add_object(Stream::new(
                    dictionary! {
                        "Type" => "CMap",
                        "CMapName" => "Mai-CMap",
                        "CIDSystemInfo" => dictionary! {
                            "Registry" => Object::string_literal("Adobe"),
                            "Ordering" => Object::string_literal(cmap_ordering),
                            "Supplement" => 0,
                        },
                        "WMode" => dictionary_wmode,
                    },
                    embedded_cmap(cmap_ordering, content_wmode),
                )))
            }
            _ => Object::Name(b"Identity-H".to_vec()),
        };
        font = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "MaiTestFont",
            "Encoding" => encoding,
            "DescendantFonts" => vec![Object::Reference(descendant)],
        };
    }
    match case {
        "font_type_missing" | "unused_invalid_font" => {
            font.remove(b"Type");
        }
        "font_type_invalid" => font.set("Type", "NotFont"),
        "font_subtype_missing" => {
            font.remove(b"Subtype");
        }
        "font_subtype_invalid" => font.set("Subtype", "UnsupportedFont"),
        "font_basefont_missing" => {
            font.remove(b"BaseFont");
        }
        "font_basefont_invalid" => font.set("BaseFont", 42),
        "font_firstchar_missing" => {
            font.remove(b"FirstChar");
        }
        "font_lastchar_missing" => {
            font.remove(b"LastChar");
        }
        "font_widths_missing" => {
            font.remove(b"Widths");
        }
        "font_widths_wrong_size" => font.set("Widths", Vec::<Object>::new()),
        "standard14_missing_metrics" => {
            font.set("Subtype", "Type1");
            font.set("BaseFont", "Helvetica");
            font.remove(b"FirstChar");
            font.remove(b"LastChar");
            font.remove(b"Widths");
            font.remove(b"FontDescriptor");
        }
        "tt_nonsymbolic_macroman" => font.set("Encoding", "MacRomanEncoding"),
        "tt_nonsymbolic_missing_encoding" => {
            font.remove(b"Encoding");
        }
        "tt_nonsymbolic_invalid_encoding" => font.set("Encoding", "StandardEncoding"),
        "tt_nonsymbolic_dictionary_winansi" => font.set(
            "Encoding",
            dictionary! {
                "Type" => "Encoding",
                "BaseEncoding" => "WinAnsiEncoding",
            },
        ),
        "tt_nonsymbolic_dictionary_macroman" => font.set(
            "Encoding",
            dictionary! {
                "Type" => "Encoding",
                "BaseEncoding" => "MacRomanEncoding",
            },
        ),
        "tt_nonsymbolic_differences" => font.set(
            "Encoding",
            dictionary! {
                "Type" => "Encoding",
                "BaseEncoding" => "WinAnsiEncoding",
                "Differences" => vec![32.into(), Object::Name(b"space".to_vec())],
            },
        ),
        "tt_symbolic_no_encoding" | "tt_symbolic_one_cmap" | "tt_symbolic_two_cmaps" => {
            font.remove(b"Encoding");
        }
        _ => {}
    }
    let font_object = if case == "direct_font" {
        Object::Dictionary(font)
    } else {
        Object::Reference(document.add_object(font))
    };

    let mut font_resources = dictionary! {
        "F1" => font_object.clone(),
    };
    if case == "repeated_aliases" {
        font_resources.set("F2", font_object.clone());
    }
    if case == "two_unembedded_fonts" {
        let descriptor = font_descriptor(&mut document, false);
        let second_descriptor = document.add_object(descriptor);
        let second_font = document.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "TrueType",
            "BaseFont" => "MaiSecondFont",
            "Encoding" => "WinAnsiEncoding",
            "FirstChar" => 32,
            "LastChar" => 32,
            "Widths" => vec![500.into()],
            "FontDescriptor" => second_descriptor,
        });
        font_resources.set("F2", second_font);
    }
    let mut resources = dictionary! {
        "Font" => dictionary! {
            "F1" => font_object,
        },
    };
    resources.set("Font", font_resources.clone());
    let page_content = match case {
        "unused_resource" | "unused_invalid_font" => Vec::new(),
        "selected_not_shown" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("ET", vec![]),
        ]),
        "unembedded_invisible" => text_content(3),
        "mixed_rendering_modes" | "mixed_visible_first" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation(
                "Tr",
                vec![if case == "mixed_visible_first" {
                    0.into()
                } else {
                    3.into()
                }],
            ),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation(
                "Tr",
                vec![if case == "mixed_visible_first" {
                    3.into()
                } else {
                    0.into()
                }],
            ),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
        ]),
        "graphics_state_visible" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tr", vec![3.into()]),
            operation("q", vec![]),
            operation("Tr", vec![0.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("Q", vec![]),
            operation("ET", vec![]),
        ]),
        "graphics_state_invisible" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tr", vec![3.into()]),
            operation("q", vec![]),
            operation("Tr", vec![0.into()]),
            operation("Q", vec![]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
        ]),
        "repeated_aliases" | "two_unembedded_fonts" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("Tf", vec![Object::Name(b"F2".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
        ]),
        "form_unembedded" | "nested_form_unembedded" => {
            let form = Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                    "Resources" => resources.clone(),
                },
                text_content(0),
            );
            let mut form_id = document.add_object(form);
            if case == "nested_form_unembedded" {
                form_id = document.add_object(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                        "Resources" => dictionary! {
                            "XObject" => dictionary! {
                                "Inner" => form_id,
                            },
                        },
                    },
                    content(vec![operation("Do", vec![Object::Name(b"Inner".to_vec())])]),
                ));
            }
            resources = dictionary! {
                "XObject" => dictionary! {
                    "Fm1" => form_id,
                },
            };
            content(vec![operation("Do", vec![Object::Name(b"Fm1".to_vec())])])
        }
        "cyclic_form" => {
            let form_id = document.new_object_id();
            document.objects.insert(
                form_id,
                Object::Stream(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                        "Resources" => dictionary! {
                            "Font" => font_resources.clone(),
                            "XObject" => dictionary! {
                                "Self" => form_id,
                            },
                        },
                    },
                    content(vec![
                        operation("BT", vec![]),
                        operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
                        operation("Tj", vec![Object::string_literal(" ")]),
                        operation("ET", vec![]),
                        operation("Do", vec![Object::Name(b"Self".to_vec())]),
                    ]),
                )),
            );
            resources = dictionary! {
                "XObject" => dictionary! {
                    "Fm1" => form_id,
                },
            };
            content(vec![operation("Do", vec![Object::Name(b"Fm1".to_vec())])])
        }
        "large_content" => {
            let mut bytes = vec![b' '; 4096];
            bytes.extend_from_slice(&text_content(0));
            bytes
        }
        "deep_graphics_state" => {
            let mut operations = Vec::new();
            operations.extend((0..5).map(|_| operation("q", vec![])));
            operations.extend((0..5).map(|_| operation("Q", vec![])));
            content(operations)
        }
        _ => text_content(0),
    };
    let contents_id = document.add_object(Stream::new(Dictionary::new(), page_content));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources,
        "Contents" => contents_id,
    };
    let inherited_resources = if case == "inherited_resources" {
        page.remove(b"Resources")
    } else {
        None
    };
    let page_id = document.add_object(page);
    let mut pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => 1,
    };
    if let Some(resources) = inherited_resources {
        pages.set("Resources", resources);
    }
    document.objects.insert(pages_id, Object::Dictionary(pages));
    let metadata_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        BASE_XMP.to_vec(),
    ));
    let intent_id = document.add_object(dictionary! {
        "Type" => "OutputIntent",
        "S" => "GTS_PDFA1",
        "OutputConditionIdentifier" => Object::string_literal("Test"),
    });
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => vec![Object::Reference(intent_id)],
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save font fixture");
    bytes
}

fn font_descriptor(document: &mut Document, embedded: bool) -> Dictionary {
    let mut descriptor = dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "MaiTestFont",
        "Flags" => 32,
        "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
        "ItalicAngle" => 0,
        "Ascent" => 800,
        "Descent" => -200,
        "CapHeight" => 700,
        "StemV" => 80,
    };
    if embedded {
        let font_file = document.add_object(Stream::new(Dictionary::new(), minimal_truetype()));
        descriptor.set("FontFile2", font_file);
    }
    descriptor
}

fn text_content(rendering_mode: i64) -> Vec<u8> {
    content(vec![
        operation("BT", vec![]),
        operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
        operation("Tr", vec![rendering_mode.into()]),
        operation("Tj", vec![Object::string_literal(" ")]),
        operation("ET", vec![]),
    ])
}

fn embedded_cmap(ordering: &str, wmode: i64) -> Vec<u8> {
    format!(
        "/CIDInit /ProcSet findresource begin\n\
         12 dict begin\n\
         begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering ({ordering}) /Supplement 0 >> def\n\
         /CMapName /Mai-CMap def\n\
         /CMapType 1 def\n\
         /WMode {wmode} def\n\
         1 begincodespacerange\n\
         <00> <FF>\n\
         endcodespacerange\n\
         1 begincidrange\n\
         <00> <FF> 0\n\
         endcidrange\n\
         endcmap\n\
         CMapName currentdict /CMap defineresource pop\n\
         end\n\
         end\n"
    )
    .into_bytes()
}

fn content(operations: Vec<Operation>) -> Vec<u8> {
    Content { operations }.encode().expect("encode content")
}

fn operation(operator: &str, operands: Vec<Object>) -> Operation {
    Operation::new(operator, operands)
}

fn minimal_truetype() -> Vec<u8> {
    minimal_truetype_with_cmap_count(1)
}

fn minimal_truetype_with_cmap_count(cmap_count: u16) -> Vec<u8> {
    let mut head = vec![0; 54];
    put_u32(&mut head, 0, 0x0001_0000);
    put_u32(&mut head, 4, 0x0001_0000);
    put_u32(&mut head, 12, 0x5F0F_3CF5);
    put_u16(&mut head, 18, 1000);
    put_i16(&mut head, 40, 500);
    put_i16(&mut head, 42, 700);
    put_u16(&mut head, 46, 8);
    put_i16(&mut head, 48, 2);

    let mut hhea = vec![0; 36];
    put_u32(&mut hhea, 0, 0x0001_0000);
    put_i16(&mut hhea, 4, 800);
    put_i16(&mut hhea, 6, -200);
    put_u16(&mut hhea, 10, 500);
    put_i16(&mut hhea, 18, 1);
    put_u16(&mut hhea, 34, 2);

    let mut maxp = vec![0; 32];
    put_u32(&mut maxp, 0, 0x0001_0000);
    put_u16(&mut maxp, 4, 2);

    let cmap_header_length = 4 + usize::from(cmap_count) * 8;
    let mut cmap = vec![0; cmap_header_length + 262];
    put_u16(&mut cmap, 2, cmap_count);
    for index in 0..usize::from(cmap_count) {
        let record = 4 + index * 8;
        put_u16(&mut cmap, record, 3);
        put_u16(
            &mut cmap,
            record + 2,
            u16::try_from(index + 1).expect("small cmap count"),
        );
        put_u32(
            &mut cmap,
            record + 4,
            u32::try_from(cmap_header_length).expect("small cmap header"),
        );
    }
    put_u16(&mut cmap, cmap_header_length, 0);
    put_u16(&mut cmap, cmap_header_length + 2, 262);
    cmap[cmap_header_length + 6 + 32] = 1;

    let family = utf16be("Mai Test");
    let postscript = utf16be("MaiTestFont");
    let mut name = vec![0; 30 + family.len() + postscript.len()];
    put_u16(&mut name, 2, 2);
    put_u16(&mut name, 4, 30);
    put_u16(&mut name, 6, 3);
    put_u16(&mut name, 8, 1);
    put_u16(&mut name, 10, 0x0409);
    put_u16(&mut name, 12, 1);
    put_u16(&mut name, 14, family.len() as u16);
    put_u16(&mut name, 18, 3);
    put_u16(&mut name, 20, 1);
    put_u16(&mut name, 22, 0x0409);
    put_u16(&mut name, 24, 6);
    put_u16(&mut name, 26, postscript.len() as u16);
    put_u16(&mut name, 28, family.len() as u16);
    name[30..30 + family.len()].copy_from_slice(&family);
    name[30 + family.len()..].copy_from_slice(&postscript);

    let mut os2 = vec![0; 78];
    put_u16(&mut os2, 2, 500);
    put_u16(&mut os2, 4, 400);
    put_u16(&mut os2, 6, 5);
    put_u16(&mut os2, 8, 0);
    put_i16(&mut os2, 68, 800);
    put_i16(&mut os2, 70, -200);
    put_u16(&mut os2, 74, 800);
    put_u16(&mut os2, 76, 200);

    let mut post = vec![0; 32];
    put_u32(&mut post, 0, 0x0003_0000);

    let mut hmtx = vec![0; 8];
    put_u16(&mut hmtx, 0, 500);
    put_u16(&mut hmtx, 4, 500);

    let tables = vec![
        (*b"OS/2", os2),
        (*b"cmap", cmap),
        (*b"glyf", vec![0; 4]),
        (*b"head", head),
        (*b"hhea", hhea),
        (*b"hmtx", hmtx),
        (*b"loca", vec![0; 6]),
        (*b"maxp", maxp),
        (*b"name", name),
        (*b"post", post),
    ];
    build_sfnt(tables)
}

fn build_sfnt(tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    let table_count = tables.len();
    let mut font = vec![0; 12 + 16 * table_count];
    put_u32(&mut font, 0, 0x0001_0000);
    put_u16(&mut font, 4, table_count as u16);
    put_u16(&mut font, 6, 128);
    put_u16(&mut font, 8, 3);
    put_u16(&mut font, 10, (table_count * 16 - 128) as u16);

    let mut head_offset = None;
    for (index, (tag, data)) in tables.iter().enumerate() {
        while !font.len().is_multiple_of(4) {
            font.push(0);
        }
        let offset = font.len();
        let directory = 12 + index * 16;
        font[directory..directory + 4].copy_from_slice(tag);
        put_u32(&mut font, directory + 4, table_checksum(data));
        put_u32(&mut font, directory + 8, offset as u32);
        put_u32(&mut font, directory + 12, data.len() as u32);
        font.extend_from_slice(data);
        if tag == b"head" {
            head_offset = Some(offset);
        }
    }
    while !font.len().is_multiple_of(4) {
        font.push(0);
    }
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(table_checksum(&font));
    put_u32(&mut font, head_offset.expect("head table") + 8, adjustment);
    font
}

fn table_checksum(bytes: &[u8]) -> u32 {
    bytes
        .chunks(4)
        .map(|chunk| {
            let mut word = [0; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            u32::from_be_bytes(word)
        })
        .fold(0u32, u32::wrapping_add)
}

fn utf16be(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn put_i16(bytes: &mut [u8], offset: usize, value: i16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
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

const EXTENSION_SCHEMA_BLOCK: &str = r#"
<rdf:Description
 xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/"
 xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#"
 xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#"
 xmlns:pdfaType="http://www.aiim.org/pdfa/ns/type#"
 xmlns:pdfaField="http://www.aiim.org/pdfa/ns/field#"
 xmlns:extensionAlias="http://www.aiim.org/pdfa/ns/extension/"
 xmlns:schemaAlias="http://www.aiim.org/pdfa/ns/schema#"
 xmlns:propertyAlias="http://www.aiim.org/pdfa/ns/property#"
 xmlns:typeAlias="http://www.aiim.org/pdfa/ns/type#"
 xmlns:fieldAlias="http://www.aiim.org/pdfa/ns/field#">
<pdfaExtension:schemas><rdf:Bag>
<rdf:li rdf:parseType="Resource">
<pdfaSchema:schema>Example schema</pdfaSchema:schema>
<pdfaSchema:namespaceURI>http://example.com/ns/</pdfaSchema:namespaceURI>
<pdfaSchema:prefix>ex</pdfaSchema:prefix>
<pdfaSchema:property><rdf:Seq>
<rdf:li rdf:parseType="Resource">
<pdfaProperty:name>example</pdfaProperty:name>
<pdfaProperty:valueType>Text</pdfaProperty:valueType>
<pdfaProperty:category>external</pdfaProperty:category>
<pdfaProperty:description>Example property</pdfaProperty:description>
</rdf:li>
</rdf:Seq></pdfaSchema:property>
<pdfaSchema:valueType><rdf:Seq>
<rdf:li rdf:parseType="Resource">
<pdfaType:type>CustomType</pdfaType:type>
<pdfaType:namespaceURI>http://example.com/type/</pdfaType:namespaceURI>
<pdfaType:prefix>extype</pdfaType:prefix>
<pdfaType:description>Example type</pdfaType:description>
<pdfaType:field><rdf:Seq>
<rdf:li rdf:parseType="Resource">
<pdfaField:name>member</pdfaField:name>
<pdfaField:valueType>Text</pdfaField:valueType>
<pdfaField:description>Example member</pdfaField:description>
</rdf:li>
</rdf:Seq></pdfaType:field>
</rdf:li>
</rdf:Seq></pdfaSchema:valueType>
</rdf:li>
</rdf:Bag></pdfaExtension:schemas>
</rdf:Description>
"#;
