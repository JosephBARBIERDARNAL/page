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
    ) {
        let descendant = document.add_object(dictionary! {
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
        });
        font = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "MaiTestFont",
            "Encoding" => "Identity-H",
            "DescendantFonts" => vec![Object::Reference(descendant)],
        };
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
        "unused_resource" => Vec::new(),
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

fn content(operations: Vec<Operation>) -> Vec<u8> {
    Content { operations }.encode().expect("encode content")
}

fn operation(operator: &str, operands: Vec<Object>) -> Operation {
    Operation::new(operator, operands)
}

fn minimal_truetype() -> Vec<u8> {
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

    let mut cmap = vec![0; 274];
    put_u16(&mut cmap, 2, 1);
    put_u16(&mut cmap, 4, 3);
    put_u16(&mut cmap, 6, 1);
    put_u32(&mut cmap, 8, 12);
    put_u16(&mut cmap, 12, 0);
    put_u16(&mut cmap, 14, 262);
    cmap[18 + 32] = 1;

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
