use std::collections::{BTreeMap, BTreeSet};

use lopdf::{Document, Object, ObjectId};

use crate::catalog::{resolve_catalog, root_reference_id};
use crate::error::PdfError;
use crate::file_spec;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{
    contains_key, dictionary_based, resolve_optional, resolved_bool, resolved_name, walk_inherited,
};
use crate::page_tree::PageEntry;
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct DocumentFeatureSummary {
    pub(crate) catalog_id: Option<PdfObjectId>,
    pub(crate) catalog_contains_lang: bool,
    pub(crate) mark_info_object_id: Option<PdfObjectId>,
    pub(crate) mark_info_is_dictionary: bool,
    pub(crate) marked: Option<bool>,
    pub(crate) suspects: Option<bool>,
    pub(crate) viewer_preferences_object_id: Option<PdfObjectId>,
    pub(crate) viewer_preferences_is_dictionary: bool,
    pub(crate) display_doc_title: Option<bool>,
    pub(crate) contains_embedded_files_name: bool,
    pub(crate) contains_optional_content: bool,
    pub(crate) file_specs_with_embedded_files: Vec<RuleFailure>,
    pub(crate) struct_tree_root_object_id: Option<PdfObjectId>,
    pub(crate) struct_tree_root_present: bool,
    pub(crate) struct_tree_root_valid: bool,
    pub(crate) struct_tree_role_map_has_cycle: bool,
    pub(crate) struct_tree_has_unmapped_type: bool,
    pub(crate) struct_tree_role_map_has_standard_remap: bool,
    pub(crate) structure_elements_missing_parent: Vec<RuleFailure>,
    pub(crate) toci_elements_not_contained_in_toc: Vec<RuleFailure>,
    pub(crate) toc_elements_with_invalid_children: Vec<RuleFailure>,
    pub(crate) actual_text_language_failures: Vec<RuleFailure>,
    pub(crate) alt_text_language_failures: Vec<RuleFailure>,
    pub(crate) expansion_text_language_failures: Vec<RuleFailure>,
    pub(crate) language_failures: Vec<RuleFailure>,
    pub(crate) language_failures_pdfa23: Vec<RuleFailure>,
    pub(crate) invalid_unicode_structure_types: Vec<RuleFailure>,
    pub(crate) invalid_page_boundaries: Vec<RuleFailure>,
    pub(crate) pages_with_pres_steps: Vec<RuleFailure>,
    pub(crate) catalog_with_requirements: Vec<RuleFailure>,
    pub(crate) catalog_with_alternate_presentations: Vec<RuleFailure>,
    pub(crate) catalog_with_needs_rendering: Vec<RuleFailure>,
    pub(crate) permissions_with_invalid_keys: Vec<RuleFailure>,
    pub(crate) signature_refs_with_digest_keys: Vec<RuleFailure>,
    pub(crate) acro_forms_with_xfa: Vec<RuleFailure>,
    pub(crate) embedded_files_with_invalid_mime: Vec<RuleFailure>,
    pub(crate) embedded_files_not_pdfa: Vec<RuleFailure>,
    pub(crate) file_specs_missing_f_or_uf: Vec<RuleFailure>,
    pub(crate) file_specs_missing_af_relationship: Vec<RuleFailure>,
    pub(crate) file_specs_not_associated: Vec<RuleFailure>,
    pub(crate) optional_content_missing_names: Vec<RuleFailure>,
    pub(crate) optional_content_duplicate_names: Vec<RuleFailure>,
    pub(crate) optional_content_invalid_orders: Vec<RuleFailure>,
    pub(crate) optional_content_as_entries: Vec<RuleFailure>,
}

type OptionalContentFailures = (
    Vec<RuleFailure>,
    Vec<RuleFailure>,
    Vec<RuleFailure>,
    Vec<RuleFailure>,
);

pub(crate) fn inspect(
    document: &Document,
    pages: &[PageEntry],
    limits: &SafetyLimits,
) -> Result<DocumentFeatureSummary, PdfError> {
    let catalog_id = root_reference_id(document);
    let Some(catalog) = resolve_catalog(document, limits)?.map(|catalog| catalog.dictionary) else {
        return Ok(DocumentFeatureSummary {
            catalog_id,
            ..DocumentFeatureSummary::default()
        });
    };

    let (mark_info_object_id, mark_info_is_dictionary, marked, suspects) = catalog
        .get(b"MarkInfo")
        .ok()
        .map(|value| -> Result<_, PdfError> {
            let object_id = value.as_reference().ok().map(Into::into);
            let resolved = resolve_optional(document, value, limits.max_reference_depth)?;
            let Some(dictionary) = resolved.and_then(|object| object.as_dict().ok()) else {
                return Ok((object_id, false, None, None));
            };
            let marked =
                resolved_bool(document, dictionary, b"Marked", limits.max_reference_depth)?;
            let suspects = resolved_bool(
                document,
                dictionary,
                b"Suspects",
                limits.max_reference_depth,
            )?;
            Ok((object_id, true, marked, suspects))
        })
        .transpose()?
        .unwrap_or((None, false, None, None));

    let (viewer_preferences_object_id, viewer_preferences_is_dictionary, display_doc_title) =
        catalog
            .get(b"ViewerPreferences")
            .ok()
            .map(|value| -> Result<_, PdfError> {
                let object_id = value.as_reference().ok().map(Into::into);
                let resolved = resolve_optional(document, value, limits.max_reference_depth)?;
                let Some(dictionary) = resolved.and_then(|object| object.as_dict().ok()) else {
                    return Ok((object_id, false, None));
                };
                let display_doc_title = resolved_bool(
                    document,
                    dictionary,
                    b"DisplayDocTitle",
                    limits.max_reference_depth,
                )?;
                Ok((object_id, true, display_doc_title))
            })
            .transpose()?
            .unwrap_or((None, false, None));

    let structure_tree = inspect_structure_tree(document, catalog, limits)?;
    let catalog_contains_lang = contains_key(catalog, b"Lang");
    let mut language_failures = Vec::new();
    let mut language_failures_pdfa23 = Vec::new();
    if let Some(failure) = crate::language::inspect_dictionary(
        document,
        limits,
        catalog,
        catalog_id,
        "document catalog",
    ) {
        language_failures.push(failure);
    }
    if let Some(failure) = crate::language::inspect_dictionary_pdfa23(
        document,
        limits,
        catalog,
        catalog_id,
        "document catalog",
    ) {
        language_failures_pdfa23.push(failure);
    }

    let names = catalog
        .get(b"Names")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|object| match object {
            Object::Dictionary(dictionary) => Some(dictionary),
            _ => None,
        });
    let contains_embedded_files_name = names.is_some_and(|names| {
        names
            .get(b"EmbeddedFiles")
            .is_ok_and(|value| !matches!(value, Object::Null))
    });
    let mut file_specs_with_embedded_files = Vec::new();
    if let Some(names) = names
        && let Ok(embedded_files) = names.get(b"EmbeddedFiles")
    {
        let mut ancestors = BTreeSet::new();
        let mut steps = 0usize;
        inspect_name_tree(
            document,
            embedded_files,
            limits,
            &mut file_specs_with_embedded_files,
            &mut ancestors,
            &mut steps,
            0,
        )?;
    }
    let contains_optional_content = catalog
        .get(b"OCProperties")
        .is_ok_and(|value| !matches!(value, Object::Null));
    let (
        optional_content_missing_names,
        optional_content_duplicate_names,
        optional_content_invalid_orders,
        optional_content_as_entries,
    ) = inspect_optional_content(document, catalog, limits)?;

    let mut invalid_page_boundaries = Vec::new();
    let mut pages_with_pres_steps = Vec::new();
    for (index, page_entry) in pages.iter().enumerate() {
        let Some(page) = page_entry.resolve(document) else {
            continue;
        };
        let object_id = page_entry.object_id().map(Into::into);
        if page
            .get(b"PresSteps")
            .is_ok_and(|value| !matches!(value, Object::Null))
        {
            pages_with_pres_steps.push(RuleFailure {
                object_id,
                description: format!("page {} contains /PresSteps", index + 1),
            });
        }
        for key in [
            b"MediaBox".as_slice(),
            b"CropBox",
            b"BleedBox",
            b"TrimBox",
            b"ArtBox",
        ] {
            if let Some((width, height)) = page_boundary_size(document, page, key, limits)?
                && (!(3.0..=14_400.0).contains(&width) || !(3.0..=14_400.0).contains(&height))
            {
                invalid_page_boundaries.push(RuleFailure {
                    object_id,
                    description: format!(
                        "page {} has an invalid /{} size",
                        index + 1,
                        String::from_utf8_lossy(key)
                    ),
                });
            }
        }
    }

    let mut catalog_with_requirements = Vec::new();
    if catalog
        .get(b"Requirements")
        .is_ok_and(|value| !matches!(value, Object::Null))
    {
        catalog_with_requirements.push(RuleFailure {
            object_id: catalog_id,
            description: "document catalog contains /Requirements".to_owned(),
        });
    }
    let mut catalog_with_alternate_presentations = Vec::new();
    if names.is_some_and(|names| {
        names
            .get(b"AlternatePresentations")
            .is_ok_and(|value| !matches!(value, Object::Null))
    }) {
        catalog_with_alternate_presentations.push(RuleFailure {
            object_id: catalog_id,
            description: "document name dictionary contains /AlternatePresentations".to_owned(),
        });
    }
    let mut catalog_with_needs_rendering = Vec::new();
    if catalog
        .get(b"NeedsRendering")
        .is_ok_and(|value| !matches!(value, Object::Boolean(false) | Object::Null))
    {
        catalog_with_needs_rendering.push(RuleFailure {
            object_id: catalog_id,
            description: "document catalog contains a non-false /NeedsRendering value".to_owned(),
        });
    }
    let mut permissions_with_invalid_keys = Vec::new();
    let mut signature_refs_with_digest_keys = Vec::new();
    let docmdp_present = catalog
        .get(b"Perms")
        .ok()
        .and_then(|value| {
            resolve_optional(document, value, limits.max_reference_depth)
                .ok()
                .flatten()
        })
        .and_then(|object| object.as_dict().ok())
        .is_some_and(|perms| perms.get(b"DocMDP").is_ok());
    if docmdp_present {
        for (object_id, object) in &document.objects {
            let Some(dictionary) = object.as_dict().ok() else {
                continue;
            };
            let Some(references) = dictionary
                .get(b"Reference")
                .ok()
                .and_then(|value| {
                    resolve_optional(document, value, limits.max_reference_depth)
                        .ok()
                        .flatten()
                })
                .and_then(|object| object.as_array().ok())
            else {
                continue;
            };
            for reference in references {
                let Some(reference) =
                    resolve_optional(document, reference, limits.max_reference_depth)?
                        .and_then(|object| object.as_dict().ok())
                else {
                    continue;
                };
                if [
                    b"DigestLocation".as_slice(),
                    b"DigestMethod",
                    b"DigestValue",
                ]
                .iter()
                .any(|key| reference.get(key).is_ok())
                {
                    signature_refs_with_digest_keys.push(RuleFailure {
                        object_id: Some((*object_id).into()),
                        description: "signature reference contains a forbidden digest key in the presence of DocMDP".to_owned(),
                    });
                }
            }
        }
    }
    if let Ok(value) = catalog.get(b"Perms") {
        let object_id = value.as_reference().ok().map(Into::into).or(catalog_id);
        if let Some(perms) = resolve_optional(document, value, limits.max_reference_depth)?
            .and_then(dictionary_based)
            && perms
                .iter()
                .any(|(key, _)| !matches!(key.as_slice(), b"UR3" | b"DocMDP"))
        {
            permissions_with_invalid_keys.push(RuleFailure {
                object_id,
                description: "permissions dictionary contains a key other than /UR3 or /DocMDP"
                    .to_owned(),
            });
        }
    }
    let mut acro_forms_with_xfa = Vec::new();
    if let Some(acro_form) = catalog
        .get(b"AcroForm")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(dictionary_based)
        && acro_form
            .get(b"XFA")
            .is_ok_and(|value| !matches!(value, Object::Null))
    {
        acro_forms_with_xfa.push(RuleFailure {
            object_id: catalog_id,
            description: "the catalog AcroForm contains /XFA".to_owned(),
        });
    }

    let mut associated_file_spec_ids = BTreeSet::new();
    collect_associated_file_spec_ids(
        document,
        catalog.get(b"AF").ok(),
        limits,
        &mut associated_file_spec_ids,
    )?;
    for page_entry in pages {
        if let Some(page) = page_entry.resolve(document) {
            collect_associated_file_spec_ids(
                document,
                page.get(b"AF").ok(),
                limits,
                &mut associated_file_spec_ids,
            )?;
            if let Some(annotations) = page
                .get(b"Annots")
                .ok()
                .map(|value| resolve_optional(document, value, limits.max_reference_depth))
                .transpose()?
                .flatten()
                .and_then(|object| object.as_array().ok())
            {
                for annotation in annotations {
                    if let Some(annotation) =
                        resolve_optional(document, annotation, limits.max_reference_depth)?
                            .and_then(dictionary_based)
                    {
                        collect_associated_file_spec_ids(
                            document,
                            annotation.get(b"AF").ok(),
                            limits,
                            &mut associated_file_spec_ids,
                        )?;
                    }
                }
            }
        }
    }
    for object in document.objects.values() {
        if let Some(dictionary) = dictionary_based(object) {
            collect_associated_file_spec_ids(
                document,
                dictionary.get(b"AF").ok(),
                limits,
                &mut associated_file_spec_ids,
            )?;
        }
    }
    let mut embedded_files_with_invalid_mime = Vec::new();
    let mut embedded_files_not_pdfa = Vec::new();
    let mut file_specs_missing_f_or_uf = Vec::new();
    let mut file_specs_missing_af_relationship = Vec::new();
    let mut file_specs_not_associated = Vec::new();
    for (object_id, object) in &document.objects {
        let Some(dictionary) = dictionary_based(object) else {
            continue;
        };
        if !contains_key(dictionary, b"EF") {
            continue;
        }
        let raw_object_id = *object_id;
        let object_id = Some(raw_object_id.into());
        if !contains_key(dictionary, b"F") || !contains_key(dictionary, b"UF") {
            file_specs_missing_f_or_uf.push(RuleFailure {
                object_id,
                description: "embedded-file specification is missing /F or /UF".to_owned(),
            });
        }
        if resolved_name(
            document,
            dictionary,
            b"AFRelationship",
            limits.max_reference_depth,
        )?
        .is_none()
        {
            file_specs_missing_af_relationship.push(RuleFailure {
                object_id,
                description: "associated-file specification is missing /AFRelationship".to_owned(),
            });
        }
        if !associated_file_spec_ids.contains(&raw_object_id) {
            file_specs_not_associated.push(RuleFailure {
                object_id,
                description: "embedded-file specification is not referenced by an /AF association"
                    .to_owned(),
            });
        }
        let Some(embedded_files) = dictionary
            .get(b"EF")
            .ok()
            .map(|value| resolve_optional(document, value, limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(dictionary_based)
        else {
            continue;
        };
        for key in [b"F".as_slice(), b"UF"] {
            let Some(value) = embedded_files
                .get(key)
                .ok()
                .map(|value| resolve_optional(document, value, limits.max_reference_depth))
                .transpose()?
                .flatten()
            else {
                continue;
            };
            let valid_mime = value
                .as_stream()
                .ok()
                .and_then(|stream| {
                    stream
                        .dict
                        .get(b"Subtype")
                        .ok()
                        .and_then(|value| value.as_name().ok())
                })
                .is_some_and(is_mime_type);
            if !valid_mime {
                embedded_files_with_invalid_mime.push(RuleFailure {
                    object_id,
                    description: format!(
                        "embedded-file stream /{} has no valid MIME /Subtype",
                        String::from_utf8_lossy(key)
                    ),
                });
            }
            let valid_pdfa = value
                .as_stream()
                .ok()
                .and_then(|stream| {
                    stream
                        .decompressed_content_with_limit(limits.max_decoded_stream_size)
                        .ok()
                })
                .is_some_and(|bytes| {
                    bytes.starts_with(b"%PDF-")
                        && [
                            crate::validation::ValidationProfile::PdfA1b,
                            crate::validation::ValidationProfile::PdfA2b,
                        ]
                        .into_iter()
                        .any(|profile| {
                            let report = crate::validation::validate_bytes_with_profile(
                                &bytes, profile, limits,
                            );
                            report.failures.is_empty() && report.checks_passed
                        })
                });
            if !valid_pdfa {
                embedded_files_not_pdfa.push(RuleFailure {
                    object_id,
                    description: format!(
                        "embedded-file stream /{} is not a valid PDF/A-1 or PDF/A-2 document",
                        String::from_utf8_lossy(key)
                    ),
                });
            }
        }
    }

    Ok(DocumentFeatureSummary {
        catalog_id,
        catalog_contains_lang,
        mark_info_object_id,
        mark_info_is_dictionary,
        marked,
        suspects,
        viewer_preferences_object_id,
        viewer_preferences_is_dictionary,
        display_doc_title,
        struct_tree_root_object_id: structure_tree.root_object_id,
        struct_tree_root_present: structure_tree.present,
        struct_tree_root_valid: structure_tree.valid,
        struct_tree_role_map_has_cycle: structure_tree.role_map_has_cycle,
        struct_tree_has_unmapped_type: structure_tree.has_unmapped_type,
        struct_tree_role_map_has_standard_remap: structure_tree.role_map_has_standard_remap,
        structure_elements_missing_parent: structure_tree.structure_elements_missing_parent,
        toci_elements_not_contained_in_toc: structure_tree.toci_elements_not_contained_in_toc,
        toc_elements_with_invalid_children: structure_tree.toc_elements_with_invalid_children,
        actual_text_language_failures: structure_tree.actual_text_language_failures,
        alt_text_language_failures: structure_tree.alt_text_language_failures,
        expansion_text_language_failures: structure_tree.expansion_text_language_failures,
        contains_embedded_files_name,
        contains_optional_content,
        file_specs_with_embedded_files,
        language_failures: structure_tree
            .language_failures
            .into_iter()
            .chain(language_failures)
            .collect(),
        language_failures_pdfa23: structure_tree
            .language_failures_pdfa23
            .into_iter()
            .chain(language_failures_pdfa23)
            .collect(),
        invalid_unicode_structure_types: structure_tree.invalid_unicode_structure_types,
        invalid_page_boundaries,
        pages_with_pres_steps,
        catalog_with_requirements,
        catalog_with_alternate_presentations,
        catalog_with_needs_rendering,
        permissions_with_invalid_keys,
        signature_refs_with_digest_keys,
        acro_forms_with_xfa,
        embedded_files_with_invalid_mime,
        embedded_files_not_pdfa,
        file_specs_missing_f_or_uf,
        file_specs_missing_af_relationship,
        file_specs_not_associated,
        optional_content_missing_names,
        optional_content_duplicate_names,
        optional_content_invalid_orders,
        optional_content_as_entries,
    })
}

fn inspect_optional_content(
    document: &Document,
    catalog: &lopdf::Dictionary,
    limits: &SafetyLimits,
) -> Result<OptionalContentFailures, PdfError> {
    let Some(properties) = catalog
        .get(b"OCProperties")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(dictionary_based)
    else {
        return Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new()));
    };
    let mut configurations = Vec::new();
    if let Some(default) = properties
        .get(b"D")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(dictionary_based)
    {
        configurations.push(default);
    }
    if let Some(values) = properties
        .get(b"Configs")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|object| object.as_array().ok())
    {
        for value in values {
            if let Some(configuration) =
                resolve_optional(document, value, limits.max_reference_depth)?
                    .and_then(dictionary_based)
            {
                configurations.push(configuration);
            }
        }
    }
    let ocg_ids = properties
        .get(b"OCGs")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|object| object.as_array().ok())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_reference().ok())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let mut names = BTreeSet::new();
    let mut missing_names = Vec::new();
    let mut duplicate_names = Vec::new();
    let mut invalid_orders = Vec::new();
    let mut as_entries = Vec::new();
    for (index, configuration) in configurations.iter().enumerate() {
        let object_id = None;
        let name = configuration_name(document, configuration, limits)?;
        match name {
            Some(name) if !name.is_empty() => {
                if !names.insert(name.clone()) {
                    duplicate_names.push(RuleFailure {
                        object_id,
                        description: format!(
                            "optional-content configuration {index} duplicates /Name {name:?}"
                        ),
                    });
                }
            }
            _ => {
                missing_names.push(RuleFailure {
                    object_id,
                    description: format!(
                        "optional-content configuration {index} has no nonempty /Name"
                    ),
                });
            }
        }
        if contains_key(configuration, b"AS") {
            as_entries.push(RuleFailure {
                object_id,
                description: format!("optional-content configuration {index} contains /AS"),
            });
        }
        if let Some(order) = configuration
            .get(b"Order")
            .ok()
            .map(|value| resolve_optional(document, value, limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|object| object.as_array().ok())
        {
            let mut ordered_ids = BTreeSet::new();
            collect_object_references(order, &mut ordered_ids);
            if ocg_ids.iter().any(|id| !ordered_ids.contains(id)) {
                invalid_orders.push(RuleFailure {
                    object_id,
                    description: format!(
                        "optional-content configuration {index} /Order omits an OCG"
                    ),
                });
            }
        }
    }
    Ok((missing_names, duplicate_names, invalid_orders, as_entries))
}

fn configuration_name(
    document: &Document,
    configuration: &lopdf::Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<String>, PdfError> {
    let Some(value) = configuration
        .get(b"Name")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
    else {
        return Ok(None);
    };
    Ok(value
        .as_str()
        .ok()
        .map(|value| String::from_utf8_lossy(value).into_owned()))
}

fn collect_object_references(values: &[Object], ids: &mut BTreeSet<ObjectId>) {
    for value in values {
        match value {
            Object::Reference(id) => {
                ids.insert(*id);
            }
            Object::Array(values) => collect_object_references(values, ids),
            _ => {}
        }
    }
}

fn collect_associated_file_spec_ids(
    document: &Document,
    value: Option<&Object>,
    limits: &SafetyLimits,
    ids: &mut BTreeSet<ObjectId>,
) -> Result<(), PdfError> {
    if let Some(value) = value
        && let Ok(id) = value.as_reference()
    {
        ids.insert(id);
    }
    let Some(value) = value
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
    else {
        return Ok(());
    };
    let Some(values) = value.as_array().ok() else {
        if let Ok(id) = value.as_reference() {
            ids.insert(id);
        }
        return Ok(());
    };
    for value in values {
        if let Ok(id) = value.as_reference() {
            ids.insert(id);
        }
    }
    Ok(())
}

fn is_mime_type(value: &[u8]) -> bool {
    let Some(separator) = value.iter().position(|byte| *byte == b'/') else {
        return false;
    };
    let (type_, subtype) = value.split_at(separator);
    let subtype = &subtype[1..];
    !type_.is_empty()
        && !subtype.is_empty()
        && type_
            .iter()
            .chain(subtype)
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'+' | b'.'))
}

fn page_boundary_size(
    document: &Document,
    page: &lopdf::Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<Option<(f64, f64)>, PdfError> {
    let Some(value) = walk_inherited(document, page, limits, key, |document, value, limits| {
        resolve_optional(document, value, limits.max_reference_depth)
    })?
    else {
        return Ok(None);
    };
    let Ok(values) = value.as_array() else {
        return Ok(Some((0.0, 0.0)));
    };
    if values.len() != 4 {
        return Ok(Some((0.0, 0.0)));
    }
    let numbers = values
        .iter()
        .map(|value| value.as_float().ok())
        .collect::<Option<Vec<_>>>();
    Ok(numbers.map(|values| {
        (
            f64::from((values[2] - values[0]).abs()),
            f64::from((values[3] - values[1]).abs()),
        )
    }))
}

#[derive(Default)]
struct StructureTreeSummary {
    root_object_id: Option<PdfObjectId>,
    present: bool,
    valid: bool,
    role_map_has_cycle: bool,
    has_unmapped_type: bool,
    role_map_has_standard_remap: bool,
    structure_elements_missing_parent: Vec<RuleFailure>,
    toci_elements_not_contained_in_toc: Vec<RuleFailure>,
    toc_elements_with_invalid_children: Vec<RuleFailure>,
    actual_text_language_failures: Vec<RuleFailure>,
    alt_text_language_failures: Vec<RuleFailure>,
    expansion_text_language_failures: Vec<RuleFailure>,
    structure_types: BTreeSet<Vec<u8>>,
    language_failures: Vec<RuleFailure>,
    language_failures_pdfa23: Vec<RuleFailure>,
    invalid_unicode_structure_types: Vec<RuleFailure>,
}

fn inspect_structure_tree(
    document: &Document,
    catalog: &lopdf::Dictionary,
    limits: &SafetyLimits,
) -> Result<StructureTreeSummary, PdfError> {
    let Ok(entry) = catalog.get(b"StructTreeRoot") else {
        return Ok(StructureTreeSummary::default());
    };
    if matches!(entry, Object::Null) {
        return Ok(StructureTreeSummary::default());
    }
    let root_object_id = entry.as_reference().ok().map(Into::into);
    let Some(root) = resolve_optional(document, entry, limits.max_reference_depth)? else {
        return Ok(StructureTreeSummary {
            root_object_id,
            present: true,
            ..StructureTreeSummary::default()
        });
    };
    let Some(root_dictionary) = root.as_dict().ok() else {
        return Ok(StructureTreeSummary {
            root_object_id,
            present: true,
            ..StructureTreeSummary::default()
        });
    };

    let mut summary = StructureTreeSummary {
        root_object_id,
        present: true,
        valid: true,
        role_map_has_cycle: false,
        has_unmapped_type: false,
        role_map_has_standard_remap: false,
        structure_elements_missing_parent: Vec::new(),
        toci_elements_not_contained_in_toc: Vec::new(),
        toc_elements_with_invalid_children: Vec::new(),
        actual_text_language_failures: Vec::new(),
        alt_text_language_failures: Vec::new(),
        expansion_text_language_failures: Vec::new(),
        structure_types: BTreeSet::new(),
        language_failures: Vec::new(),
        language_failures_pdfa23: Vec::new(),
        invalid_unicode_structure_types: Vec::new(),
    };
    let role_map = root_dictionary
        .get(b"RoleMap")
        .ok()
        .map(|value| inspect_role_map(document, value, limits))
        .transpose()?
        .unwrap_or_default();
    summary.role_map_has_cycle = role_map.has_cycle;
    summary.role_map_has_standard_remap = role_map.has_standard_remap;
    let mut ancestors = BTreeSet::new();
    if let Ok(root_id) = entry.as_reference() {
        ancestors.insert(root_id);
    }
    let mut steps = 0;
    if let Ok(kids) = root_dictionary.get(b"K") {
        inspect_structure_kids(
            document,
            kids,
            limits,
            &mut summary,
            &mut ancestors,
            &mut steps,
            0,
            false,
            StructureTraversalContext {
                parent_standard_type: None,
                role_map: &role_map.mappings,
            },
        )?;
    }
    summary.has_unmapped_type = summary.structure_types.iter().any(|structure_type| {
        !is_standard_structure_type(structure_type)
            && !resolves_to_standard_type(
                structure_type,
                &role_map.mappings,
                limits.max_object_count,
            )
    });
    Ok(summary)
}

#[derive(Default)]
struct RoleMapSummary {
    mappings: BTreeMap<Vec<u8>, Vec<u8>>,
    has_cycle: bool,
    has_standard_remap: bool,
}

fn inspect_role_map(
    document: &Document,
    value: &Object,
    limits: &SafetyLimits,
) -> Result<RoleMapSummary, PdfError> {
    let role_map = match resolve_optional(document, value, limits.max_reference_depth) {
        Ok(Some(object)) => dictionary_based(object),
        Ok(None) | Err(PdfError::ReferenceDepth(_)) => None,
        Err(error) => return Err(error),
    };
    let Some(role_map) = role_map else {
        return Ok(RoleMapSummary::default());
    };

    let mut mappings = BTreeMap::new();
    for (entries, (source, target)) in role_map.iter().enumerate() {
        if entries >= limits.max_object_count {
            break;
        }
        let target = match resolve_optional(document, target, limits.max_reference_depth) {
            Ok(Some(object)) => object.as_name().ok(),
            Ok(None) | Err(PdfError::ReferenceDepth(_)) => None,
            Err(error) => return Err(error),
        };
        let Some(target) = target else {
            continue;
        };
        mappings.insert(source.to_vec(), target.to_vec());
    }

    let mut summary = RoleMapSummary {
        mappings,
        has_cycle: false,
        has_standard_remap: false,
    };
    summary.has_standard_remap = summary
        .mappings
        .iter()
        .any(|(source, target)| is_standard_structure_type(source) && source != target);
    for source in summary.mappings.keys() {
        let mut path = BTreeSet::new();
        let mut current = source.as_slice();
        let mut steps = 0usize;
        loop {
            if steps >= limits.max_object_count {
                break;
            }
            steps += 1;
            if !path.insert(current.to_vec()) {
                summary.has_cycle = true;
                break;
            }
            let Some(target) = summary.mappings.get(current) else {
                break;
            };
            current = target;
        }
    }
    Ok(summary)
}

fn resolves_to_standard_type(
    source: &[u8],
    mappings: &BTreeMap<Vec<u8>, Vec<u8>>,
    max_steps: usize,
) -> bool {
    let mut current = source;
    let mut steps = 0;
    while steps < max_steps {
        if is_standard_structure_type(current) {
            return true;
        }
        let Some(target) = mappings.get(current) else {
            return false;
        };
        current = target;
        steps += 1;
    }
    // A configured bound is an inspection limit, not evidence that the role
    // is invalid. The caller reports only mappings that were fully resolved.
    true
}

fn resolved_standard_type<'a>(
    source: &'a [u8],
    mappings: &'a BTreeMap<Vec<u8>, Vec<u8>>,
    max_steps: usize,
) -> Option<&'a [u8]> {
    let mut current = source;
    for _ in 0..max_steps {
        if is_standard_structure_type(current) {
            return Some(current);
        }
        current = mappings.get(current)?.as_slice();
    }
    None
}

fn is_standard_structure_type(value: &[u8]) -> bool {
    matches!(
        value,
        b"Document"
            | b"Part"
            | b"Art"
            | b"Sect"
            | b"Div"
            | b"BlockQuote"
            | b"Caption"
            | b"TOC"
            | b"TOCI"
            | b"Index"
            | b"NonStruct"
            | b"Private"
            | b"P"
            | b"H"
            | b"H1"
            | b"H2"
            | b"H3"
            | b"H4"
            | b"H5"
            | b"H6"
            | b"L"
            | b"LI"
            | b"Lbl"
            | b"LBody"
            | b"Table"
            | b"TR"
            | b"TH"
            | b"TD"
            | b"THead"
            | b"TBody"
            | b"TFoot"
            | b"Span"
            | b"Quote"
            | b"Note"
            | b"Reference"
            | b"BibEntry"
            | b"Code"
            | b"Link"
            | b"Annot"
            | b"Ruby"
            | b"RB"
            | b"RT"
            | b"RP"
            | b"Warichu"
            | b"WT"
            | b"WP"
            | b"Figure"
            | b"Formula"
            | b"Form"
    )
}

#[derive(Clone, Copy)]
struct StructureTraversalContext<'parent, 'role_map> {
    parent_standard_type: Option<&'parent [u8]>,
    role_map: &'role_map BTreeMap<Vec<u8>, Vec<u8>>,
}

fn inspect_structure_kids(
    document: &Document,
    value: &Object,
    limits: &SafetyLimits,
    summary: &mut StructureTreeSummary,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    depth: usize,
    parent_has_lang: bool,
    context: StructureTraversalContext<'_, '_>,
) -> Result<(), PdfError> {
    if depth > limits.max_reference_depth {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    *steps += 1;
    if *steps > limits.max_object_count {
        return Err(PdfError::TooManyObjects {
            actual: *steps,
            limit: limits.max_object_count,
        });
    }
    let Some(resolved) = resolve_optional(document, value, limits.max_reference_depth)? else {
        return Ok(());
    };
    match resolved {
        Object::Integer(_) => {}
        Object::Array(values) => {
            for value in values {
                inspect_structure_kids(
                    document,
                    value,
                    limits,
                    summary,
                    ancestors,
                    steps,
                    depth + 1,
                    parent_has_lang,
                    context,
                )?;
            }
        }
        Object::Dictionary(dictionary) => {
            let structure_id = value.as_reference().ok();
            if let Some(structure_id) = structure_id
                && !ancestors.insert(structure_id)
            {
                return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
            }
            let result = inspect_structure_element(
                document,
                dictionary,
                structure_id.map(Into::into),
                limits,
                summary,
                ancestors,
                steps,
                depth,
                parent_has_lang,
                context,
            );
            if let Some(structure_id) = structure_id {
                ancestors.remove(&structure_id);
            }
            result?;
        }
        _ => {}
    }
    Ok(())
}

fn inspect_structure_element(
    document: &Document,
    dictionary: &lopdf::Dictionary,
    object_id: Option<PdfObjectId>,
    limits: &SafetyLimits,
    summary: &mut StructureTreeSummary,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    depth: usize,
    parent_has_lang: bool,
    context: StructureTraversalContext<'_, '_>,
) -> Result<(), PdfError> {
    if let Some(failure) = crate::language::inspect_dictionary(
        document,
        limits,
        dictionary,
        object_id,
        "structure element",
    ) {
        summary.language_failures.push(failure);
    }
    if let Some(failure) = crate::language::inspect_dictionary_pdfa23(
        document,
        limits,
        dictionary,
        object_id,
        "structure element",
    ) {
        summary.language_failures_pdfa23.push(failure);
    }
    let Some(structure_type) = dictionary
        .get(b"S")
        .ok()
        .and_then(|value| value.as_name().ok())
    else {
        let kind = dictionary
            .get(b"Type")
            .ok()
            .and_then(|value| value.as_name().ok());
        if matches!(kind, Some(b"MCR") | Some(b"OBJR")) {
            return Ok(());
        }
        return Ok(());
    };
    summary.structure_types.insert(structure_type.to_vec());
    if !contains_key(dictionary, b"P") {
        summary.structure_elements_missing_parent.push(RuleFailure {
            object_id,
            description: "a structure element dictionary does not contain the /P parent entry"
                .to_owned(),
        });
    }
    if structure_type == b"TOCI"
        && context
            .parent_standard_type
            .is_none_or(|parent| parent != b"TOC")
    {
        summary
            .toci_elements_not_contained_in_toc
            .push(RuleFailure {
                object_id,
                description: "a TOCI structure element is not contained in a TOC structure element"
                    .to_owned(),
            });
    }
    let resolved_type =
        resolved_standard_type(structure_type, context.role_map, limits.max_object_count);
    if context.parent_standard_type == Some(b"TOC".as_slice())
        && !matches!(resolved_type, Some(b"TOC" | b"TOCI" | b"Caption"))
    {
        summary
            .toc_elements_with_invalid_children
            .push(RuleFailure {
                object_id,
                description:
                    "a TOC structure element contains a child other than TOC, TOCI, or Caption"
                        .to_owned(),
            });
    }
    if !crate::unicode_names::is_valid_utf8(structure_type) {
        summary.invalid_unicode_structure_types.push(RuleFailure {
            object_id,
            description: "a structure element /S name is not valid UTF-8".to_owned(),
        });
    }
    let contains_lang = contains_key(dictionary, b"Lang");
    if has_text_attribute(document, dictionary, limits, b"ActualText")?
        && !contains_lang
        && !parent_has_lang
    {
        summary.actual_text_language_failures.push(RuleFailure {
            object_id,
            description:
                "a structure element /ActualText string has no local, inherited, or catalog /Lang"
                    .to_owned(),
        });
    }
    if has_text_attribute(document, dictionary, limits, b"Alt")?
        && !contains_lang
        && !parent_has_lang
    {
        summary.alt_text_language_failures.push(RuleFailure {
            object_id,
            description:
                "a structure element /Alt string has no local, inherited, or catalog /Lang"
                    .to_owned(),
        });
    }
    if has_text_attribute(document, dictionary, limits, b"E")? && !contains_lang && !parent_has_lang
    {
        summary.expansion_text_language_failures.push(RuleFailure {
            object_id,
            description: "a structure element /E string has no local, inherited, or catalog /Lang"
                .to_owned(),
        });
    }
    if let Ok(kids) = dictionary.get(b"K") {
        inspect_structure_kids(
            document,
            kids,
            limits,
            summary,
            ancestors,
            steps,
            depth + 1,
            parent_has_lang || contains_lang,
            StructureTraversalContext {
                parent_standard_type: resolved_standard_type(
                    structure_type,
                    context.role_map,
                    limits.max_object_count,
                ),
                role_map: context.role_map,
            },
        )?;
    }
    Ok(())
}

fn has_text_attribute(
    document: &Document,
    dictionary: &lopdf::Dictionary,
    limits: &SafetyLimits,
    key: &[u8],
) -> Result<bool, PdfError> {
    let Some(value) = dictionary.get(key).ok() else {
        return Ok(false);
    };
    Ok(matches!(
        resolve_optional(document, value, limits.max_reference_depth)?,
        Some(Object::String(_, _))
    ))
}

/// Walks one name-tree node (an intermediate node with `/Kids`, a leaf with
/// `/Names`, or both), starting from `node` before it has been resolved so
/// that a node's own indirect identity is tracked for cycle detection just
/// like every other reference this node is reached through — including the
/// tree's own root, unlike a walker that only registers references
/// encountered while iterating `/Kids`. This mirrors `page_tree::collect_pages`'s
/// traversal shape so both trees share one cycle-safety story: `ancestors`
/// tracks only ids currently on the path from the root to this call (pushed
/// on entry, popped before every return), so a *true* cycle (an id
/// revisited while still an ancestor) raises `PdfError::ReferenceDepth`,
/// while the same file specification or intermediate node legitimately
/// reachable from two different `Kids` branches (a DAG, not a cycle) is not
/// mistaken for one — confirmed against veraPDF 1.30.2, which processes
/// such a shared reference without a parse or resource-limit failure, the
/// same way it does for a page shared by two `Pages` branches. `steps`
/// bounds the walk's total work (independent of ancestor depth) against a
/// DAG that fans out shared subtrees without any node being its own
/// ancestor, the same DAG-blowup safety `page_tree.rs` uses.
fn inspect_name_tree(
    document: &Document,
    node: &Object,
    limits: &SafetyLimits,
    failures: &mut Vec<RuleFailure>,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    depth: usize,
) -> Result<(), PdfError> {
    if depth > limits.max_reference_depth {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    *steps += 1;
    if *steps > limits.max_object_count {
        return Err(PdfError::TooManyObjects {
            actual: *steps,
            limit: limits.max_object_count,
        });
    }
    let object_id = node.as_reference().ok();
    if let Some(id) = object_id
        && !ancestors.insert(id)
    {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    let result = inspect_name_tree_node(document, node, limits, failures, ancestors, steps, depth);
    if let Some(id) = object_id {
        ancestors.remove(&id);
    }
    result
}

fn inspect_name_tree_node(
    document: &Document,
    node: &Object,
    limits: &SafetyLimits,
    failures: &mut Vec<RuleFailure>,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    depth: usize,
) -> Result<(), PdfError> {
    let Some(node) =
        resolve_optional(document, node, limits.max_reference_depth)?.and_then(dictionary_based)
    else {
        return Ok(());
    };
    if let Ok(names) = node.get(b"Names")
        && let Some(names) = resolve_optional(document, names, limits.max_reference_depth)?
            .and_then(|object| object.as_array().ok())
    {
        for value in names.iter().skip(1).step_by(2) {
            if let Some(failure) = file_spec::inspect(
                document,
                value,
                limits,
                "file specification in the EmbeddedFiles name tree",
            )? {
                failures.push(failure);
            }
        }
    }
    if let Ok(kids) = node.get(b"Kids")
        && let Some(kids) = resolve_optional(document, kids, limits.max_reference_depth)?
            .and_then(|object| object.as_array().ok())
    {
        for value in kids {
            inspect_name_tree(
                document,
                value,
                limits,
                failures,
                ancestors,
                steps,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use lopdf::{Document, Object, dictionary};

    use super::inspect;
    use crate::{PdfError, SafetyLimits};

    #[test]
    fn rejects_cyclic_embedded_files_name_tree() {
        let mut document = Document::with_version("1.4");
        document.trailer.set("Root", Object::Reference((1, 0)));
        document.objects.insert(
            (1, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "Names" => Object::Reference((2, 0)),
            }),
        );
        document.objects.insert(
            (2, 0),
            Object::Dictionary(dictionary! { "EmbeddedFiles" => Object::Reference((3, 0)) }),
        );
        document.objects.insert(
            (3, 0),
            Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference((4, 0))] }),
        );
        document.objects.insert(
            (4, 0),
            Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference((3, 0))] }),
        );

        assert!(matches!(
            inspect(&document, &[], &SafetyLimits::default()),
            Err(PdfError::ReferenceDepth(_))
        ));
    }

    #[test]
    fn rejects_an_embedded_files_node_that_references_itself() {
        let mut document = Document::with_version("1.4");
        document.trailer.set("Root", Object::Reference((1, 0)));
        document.objects.insert(
            (1, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "Names" => Object::Reference((2, 0)),
            }),
        );
        document.objects.insert(
            (2, 0),
            Object::Dictionary(dictionary! { "EmbeddedFiles" => Object::Reference((3, 0)) }),
        );
        // The EmbeddedFiles node's own Kids loops straight back to itself,
        // a one-hop self-reference at the tree's root rather than a cycle
        // spanning several Kids hops.
        document.objects.insert(
            (3, 0),
            Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference((3, 0))] }),
        );

        assert!(matches!(
            inspect(&document, &[], &SafetyLimits::default()),
            Err(PdfError::ReferenceDepth(_))
        ));
    }

    #[test]
    fn accepts_a_valid_structure_element_tree() {
        let mut document = Document::with_version("1.4");
        document.trailer.set("Root", Object::Reference((1, 0)));
        document.objects.insert(
            (1, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "StructTreeRoot" => Object::Reference((2, 0)),
            }),
        );
        document.objects.insert(
            (2, 0),
            Object::Dictionary(dictionary! { "K" => vec![Object::Reference((3, 0))] }),
        );
        document.objects.insert(
            (3, 0),
            Object::Dictionary(dictionary! {
                "S" => "P",
                "P" => Object::Reference((2, 0)),
                "K" => vec![Object::Reference((4, 0))],
            }),
        );
        document
            .objects
            .insert((4, 0), Object::Dictionary(dictionary! { "S" => "Span" }));

        let features = inspect(&document, &[], &SafetyLimits::default()).expect("inspect");
        assert!(features.struct_tree_root_present);
        assert!(features.struct_tree_root_valid);
    }

    /// Confirmed against veraPDF 1.30.2: the same name-tree leaf reachable
    /// from two different `Kids` branches (a DAG, not a cycle — neither
    /// branch is the other's ancestor) is processed without error, the same
    /// way a Page object shared by two `Pages` branches is (see
    /// `page_tree.rs`'s identical fix this session).
    #[test]
    fn shared_name_tree_leaf_reached_from_two_branches_is_not_a_cycle() {
        let mut document = Document::with_version("1.4");
        document.trailer.set("Root", Object::Reference((1, 0)));
        document.objects.insert(
            (1, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "Names" => Object::Reference((2, 0)),
            }),
        );
        document.objects.insert(
            (2, 0),
            Object::Dictionary(dictionary! { "EmbeddedFiles" => Object::Reference((3, 0)) }),
        );
        // Root EmbeddedFiles node with two Kids branches (4,0) and (5,0),
        // both pointing at the same leaf (6,0).
        document.objects.insert(
            (3, 0),
            Object::Dictionary(
                dictionary! { "Kids" => vec![Object::Reference((4, 0)), Object::Reference((5, 0))] },
            ),
        );
        document.objects.insert(
            (4, 0),
            Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference((6, 0))] }),
        );
        document.objects.insert(
            (5, 0),
            Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference((6, 0))] }),
        );
        document.objects.insert(
            (6, 0),
            Object::Dictionary(dictionary! {
                "Names" => vec![
                    Object::string_literal("file"),
                    Object::Dictionary(dictionary! {}),
                ],
            }),
        );

        assert!(inspect(&document, &[], &SafetyLimits::default()).is_ok());
    }
}
