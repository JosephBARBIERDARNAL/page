use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, LoadOptions, Object, ObjectId};
use serde::Serialize;

use crate::catalog::{resolve_catalog, root_reference_id};
use crate::error::PdfError;
use crate::font_embedding::{self, FontEmbeddingSummary};
use crate::limits::SafetyLimits;
use crate::metadata::{DocumentMetadata, XmpMetadata, parse_xmp};
use crate::object_resolution::{contains_key, dictionary_based, resolve, resolve_optional};
use crate::page_tree;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct PdfObjectId {
    pub object_number: u32,
    pub generation: u16,
}

impl From<ObjectId> for PdfObjectId {
    fn from((object_number, generation): ObjectId) -> Self {
        Self {
            object_number,
            generation,
        }
    }
}

/// A coarse, whole-document informational font count — not a conformance
/// predicate. See `font_is_embedded` in this module for why this is
/// intentionally distinct from the pinned `PDFA1B-FONT-EMBEDDING-001` rule.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FontSummary {
    pub total: usize,
    pub embedded: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CatalogMetadataStream {
    pub present: bool,
    pub is_stream: bool,
    pub type_is_metadata: bool,
    pub subtype_is_xml: bool,
    pub has_filter: bool,
}

impl CatalogMetadataStream {
    /// Whether the catalog Metadata entry resolves to a stream with
    /// `/Type /Metadata` and `/Subtype /XML`, as PDF/A-1b requires.
    pub fn is_valid(&self) -> bool {
        self.present && self.is_stream && self.type_is_metadata && self.subtype_is_xml
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct IccHeader {
    pub device_class: String,
    pub color_space: String,
    pub version_major: u8,
    pub version_minor: u8,
}

impl IccHeader {
    const REQUIRED_LENGTH: usize = 20;

    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::REQUIRED_LENGTH {
            return None;
        }
        let device_class = bytes.get(12..16)?;
        let color_space = bytes.get(16..20)?;
        Some(Self {
            device_class: signature(device_class),
            color_space: signature(color_space),
            version_major: *bytes.get(8)?,
            version_minor: *bytes.get(9)? >> 4,
        })
    }

    pub fn conforms_to_pdfa_1_output_intent(&self) -> bool {
        matches!(self.device_class.as_str(), "prtr" | "mntr")
            && matches!(self.color_space.as_str(), "RGB " | "CMYK" | "GRAY")
            && self.version_major < 3
    }

    pub fn conforms_to_pdfa_2_output_intent(&self) -> bool {
        matches!(self.device_class.as_str(), "prtr" | "mntr")
            && matches!(self.color_space.as_str(), "RGB " | "CMYK" | "GRAY")
            && self.version_major < 5
    }

    pub(crate) fn conforms_to_pdfa_1_input_profile(&self) -> bool {
        matches!(
            self.device_class.as_str(),
            "prtr" | "mntr" | "scnr" | "spac"
        ) && matches!(self.color_space.as_str(), "RGB " | "CMYK" | "GRAY" | "Lab ")
            && self.version_major < 3
    }

    pub(crate) fn conforms_to_pdfa_2_input_profile(&self) -> bool {
        matches!(
            self.device_class.as_str(),
            "prtr" | "mntr" | "scnr" | "spac"
        ) && matches!(self.color_space.as_str(), "RGB " | "CMYK" | "GRAY" | "Lab ")
            && self.version_major < 5
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct OutputIntentSummary {
    pub object_id: Option<PdfObjectId>,
    pub is_dictionary_based: bool,
    pub subtype_present: bool,
    pub subtype: Option<String>,
    pub dest_output_profile_present: bool,
    pub dest_output_profile_ref_present: bool,
    pub dest_output_profile_id: Option<PdfObjectId>,
    pub dest_output_profile_is_stream: bool,
    pub dest_output_profile_header: Option<IccHeader>,
    pub dest_output_profile_decode_error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct OutputIntentsSummary {
    pub present: bool,
    pub is_array: bool,
    pub entries: Vec<OutputIntentSummary>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PdfDocument {
    pub version: String,
    pub encrypted: bool,
    #[serde(skip)]
    pub(crate) encryption_permissions: Option<u64>,
    #[serde(skip)]
    pub(crate) encryption_dictionary_object: Option<PdfObjectId>,
    #[serde(skip)]
    pub(crate) encrypted_content_unavailable: bool,
    pub catalog_reference: Option<PdfObjectId>,
    pub catalog_present: bool,
    pub page_count: usize,
    pub trailer_keys: Vec<String>,
    pub trailer_id: Option<Vec<Vec<u8>>>,
    pub info: DocumentMetadata,
    pub info_object: Option<PdfObjectId>,
    pub xmp: Option<XmpMetadata>,
    pub xmp_object: Option<PdfObjectId>,
    pub xmp_parse_error: Option<String>,
    pub catalog_metadata: CatalogMetadataStream,
    /// Legacy array-entry identities retained for report compatibility.
    pub output_intents: Vec<Option<PdfObjectId>>,
    pub output_intents_summary: OutputIntentsSummary,
    pub fonts: FontSummary,
    pub object_count: usize,
}

pub(crate) struct InspectionSummary {
    pub(crate) header: crate::syntax::HeaderSummary,
    pub(crate) content: crate::content_support::ContentExecutionSummary,
    pub(crate) font_embedding: FontEmbeddingSummary,
    pub(crate) icc_based: crate::icc_based::IccBasedSummary,
    pub(crate) xobjects: crate::xobject::XObjectSummary,
    pub(crate) graphics: crate::graphics::GraphicsSummary,
    pub(crate) annotations: crate::annotations::AnnotationSummary,
    pub(crate) actions: crate::actions::ActionSummary,
    pub(crate) forms: crate::forms::FormSummary,
    pub(crate) document_features: crate::document_features::DocumentFeatureSummary,
    pub(crate) object_limits: crate::object_limits::ObjectLimitsSummary,
    pub(crate) stream_safety: crate::stream_safety::StreamSafetySummary,
    pub(crate) unicode_names: crate::unicode_names::UnicodeNameSummary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InspectionNeed {
    Required,
    NotApplicable,
    Unknown,
}

impl InspectionNeed {
    pub(crate) const fn should_run(self) -> bool {
        !matches!(self, Self::NotApplicable)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InspectionPlan {
    pub(crate) font_details: InspectionNeed,
    pub(crate) xobjects: InspectionNeed,
    pub(crate) annotations: InspectionNeed,
    pub(crate) forms: InspectionNeed,
    pub(crate) actions: InspectionNeed,
}

impl InspectionPlan {
    pub(crate) const fn all() -> Self {
        Self {
            font_details: InspectionNeed::Unknown,
            xobjects: InspectionNeed::Unknown,
            annotations: InspectionNeed::Unknown,
            forms: InspectionNeed::Unknown,
            actions: InspectionNeed::Unknown,
        }
    }

    pub(crate) fn after_content_discovery(
        self,
        font_usage_present: bool,
        xobject_usage_present: bool,
        annotation_present: bool,
        form_candidate_present: bool,
        action_candidate_present: bool,
    ) -> Self {
        Self {
            font_details: match (self.font_details, font_usage_present) {
                (InspectionNeed::NotApplicable, _) => InspectionNeed::NotApplicable,
                (_, true) => InspectionNeed::Required,
                (_, false) => InspectionNeed::NotApplicable,
            },
            xobjects: match (self.xobjects, xobject_usage_present) {
                (InspectionNeed::NotApplicable, _) => InspectionNeed::NotApplicable,
                (_, true) => InspectionNeed::Required,
                (_, false) => InspectionNeed::NotApplicable,
            },
            annotations: match (self.annotations, annotation_present) {
                (InspectionNeed::NotApplicable, _) => InspectionNeed::NotApplicable,
                (_, true) => InspectionNeed::Required,
                (_, false) => InspectionNeed::NotApplicable,
            },
            forms: match (self.forms, form_candidate_present) {
                (InspectionNeed::NotApplicable, _) => InspectionNeed::NotApplicable,
                (_, true) => InspectionNeed::Required,
                (_, false) => InspectionNeed::NotApplicable,
            },
            actions: match (self.actions, action_candidate_present) {
                (InspectionNeed::NotApplicable, _) => InspectionNeed::NotApplicable,
                (_, true) => InspectionNeed::Required,
                (_, false) => InspectionNeed::NotApplicable,
            },
        }
    }
}

pub(crate) struct ValidationPreparation {
    document: Document,
    pages: Option<Vec<page_tree::PageEntry>>,
    normalized: PdfDocument,
}

impl PdfDocument {
    pub fn from_bytes(bytes: &[u8], limits: &SafetyLimits) -> Result<Self, PdfError> {
        Ok(Self::prepare_for_validation(bytes, limits)?.normalized)
    }

    pub(crate) fn prepare_for_validation(
        bytes: &[u8],
        limits: &SafetyLimits,
    ) -> Result<ValidationPreparation, PdfError> {
        let document = load_document(bytes, limits)?;
        let (_, encrypted_content_unavailable) = encryption_status(&document);
        if !encrypted_content_unavailable {
            enforce_object_limit(&document, limits)?;
        }
        let pages = if encrypted_content_unavailable {
            None
        } else {
            Some(match resolve_catalog(&document, limits)? {
                Some(catalog) => page_tree::collect_pages(&document, catalog.dictionary, limits)?,
                None => Vec::new(),
            })
        };
        let normalized = Self::normalize(&document, limits, pages.as_ref().map(Vec::len))?;
        Ok(ValidationPreparation {
            document,
            pages,
            normalized,
        })
    }

    pub(crate) fn from_bytes_with_inspections(
        bytes: &[u8],
        limits: &SafetyLimits,
    ) -> Result<(Self, InspectionSummary), PdfError> {
        Self::prepare_for_validation(bytes, limits)?.into_inspections(bytes, limits)
    }

    fn normalize(
        document: &Document,
        limits: &SafetyLimits,
        collected_page_count: Option<usize>,
    ) -> Result<Self, PdfError> {
        let catalog_reference = root_reference_id(document);
        let (encrypted, encrypted_content_unavailable) = encryption_status(document);
        let (encryption_permissions, encryption_dictionary_object) =
            encryption_permissions(document);
        let mut trailer_keys = document
            .trailer
            .iter()
            .map(|(key, _)| String::from_utf8_lossy(key).into_owned())
            .collect::<Vec<_>>();
        trailer_keys.sort();
        let trailer_id = extract_trailer_id(document);

        // A password-protected document may expose its trailer while keeping
        // referenced objects inaccessible. Encryption alone is enough to fail
        // PDF/A-1b, so retain the available structure without treating those
        // inaccessible objects as a syntax error.
        if encrypted_content_unavailable {
            return Ok(Self {
                version: document.version.clone(),
                encrypted: true,
                encryption_permissions,
                encryption_dictionary_object,
                encrypted_content_unavailable: true,
                catalog_reference,
                trailer_keys,
                trailer_id,
                object_count: document.objects.len(),
                ..Self::default()
            });
        }

        let catalog = resolve_catalog(document, limits)?;
        let catalog = catalog.map(|catalog| catalog.dictionary);

        let (info, info_object) = extract_info(document, limits)?;
        let catalog_metadata = inspect_catalog_metadata(document, catalog, limits)?;
        let XmpExtraction {
            xmp,
            object_id: xmp_object,
            parse_error: xmp_parse_error,
        } = extract_xmp(document, catalog, limits)?;
        let output_intents_summary = extract_output_intents(document, catalog, limits)?;
        let output_intents = output_intents_summary
            .entries
            .iter()
            .map(|entry| entry.object_id)
            .collect();
        let page_count = match collected_page_count {
            Some(count) => count,
            None => match catalog {
                Some(catalog) => page_tree::collect_pages(document, catalog, limits)?.len(),
                None => 0,
            },
        };

        Ok(Self {
            version: document.version.clone(),
            encrypted,
            encryption_permissions,
            encryption_dictionary_object,
            encrypted_content_unavailable: false,
            catalog_reference,
            catalog_present: catalog.is_some(),
            page_count,
            trailer_keys,
            trailer_id,
            info,
            info_object,
            xmp,
            xmp_object,
            xmp_parse_error,
            catalog_metadata,
            output_intents,
            output_intents_summary,
            fonts: summarize_fonts(document, limits)?,
            object_count: document.objects.len(),
        })
    }
}

impl ValidationPreparation {
    pub(crate) fn document(&self) -> &PdfDocument {
        &self.normalized
    }

    pub(crate) fn with_syntax(
        self,
        bytes: &[u8],
        limits: &SafetyLimits,
    ) -> Result<(Self, crate::syntax::SyntaxSummary), PdfError> {
        let syntax = crate::syntax::inspect(bytes, &self.document, limits)?;
        Ok((self, syntax))
    }

    pub(crate) fn into_inspections(
        self,
        bytes: &[u8],
        limits: &SafetyLimits,
    ) -> Result<(PdfDocument, InspectionSummary), PdfError> {
        let (preparation, syntax) = self.with_syntax(bytes, limits)?;
        preparation.into_inspections_with_syntax(bytes, limits, syntax, InspectionPlan::all())
    }

    pub(crate) fn into_inspections_with_syntax(
        self,
        bytes: &[u8],
        limits: &SafetyLimits,
        syntax: crate::syntax::SyntaxSummary,
        plan: InspectionPlan,
    ) -> Result<(PdfDocument, InspectionSummary), PdfError> {
        let Self {
            document,
            pages,
            normalized,
        } = self;
        let header = syntax.header.clone();
        let inspections = if normalized.encrypted_content_unavailable {
            InspectionSummary {
                header,
                content: crate::content_support::ContentExecutionSummary::default(),
                font_embedding: FontEmbeddingSummary::default(),
                icc_based: crate::icc_based::IccBasedSummary::default(),
                xobjects: crate::xobject::XObjectSummary::default(),
                graphics: crate::graphics::GraphicsSummary::default(),
                annotations: crate::annotations::AnnotationSummary::default(),
                actions: crate::actions::ActionSummary::default(),
                forms: crate::forms::FormSummary::default(),
                document_features: crate::document_features::DocumentFeatureSummary::default(),
                object_limits: crate::object_limits::ObjectLimitsSummary::default(),
                stream_safety: crate::stream_safety::StreamSafetySummary::default(),
                unicode_names: crate::unicode_names::UnicodeNameSummary::default(),
            }
        } else {
            // Computed once and shared: every inspector below that walks the
            // page tree (icc_based, graphics, annotations, actions, forms,
            // font_embedding) previously called `document.get_pages()`
            // independently, repeating the same page-tree traversal up to
            // six times per document. `page_tree::collect_pages` also
            // replaces `lopdf`'s own bound (a hardcoded depth and object-count
            // iteration budget) with this crate's `SafetyLimits`, and
            // surfaces a cyclic or overlong page tree as `PdfError`
            // instead of silently truncating the page list.
            let pages = pages.unwrap_or_default();
            // One shared execution establishes the exact resource population
            // used by colour, XObject, graphics, and font rule predicates.
            let document_features = crate::document_features::inspect(&document, &pages, limits)?;
            let mut content_cache = crate::content_support::ContentCache::new();
            let content = crate::content_support::execute_content(
                &document,
                &pages,
                &mut content_cache,
                &document_features.tagged_text_language,
                limits,
            )?;
            let plan = plan.after_content_discovery(
                !content.fonts.is_empty(),
                !content.xobjects.is_empty(),
                content.has_annotations,
                document_features.catalog_has_acro_form || content.has_widget_annotations,
                document_features.catalog_has_action_candidates
                    || content.has_page_or_annotation_actions,
            );
            let icc_based = crate::icc_based::inspect(&document, &content, limits)?;
            let xobjects = crate::xobject::inspect(&document, &content, limits, plan.xobjects)?;
            let graphics = crate::graphics::inspect(&document, &content, &pages, limits)?;
            let actions = crate::actions::inspect(&document, &pages, limits, plan.actions)?;
            let forms = crate::forms::inspect(&document, &pages, limits, plan.forms)?;
            let annotations = crate::annotations::inspect(
                &document,
                &pages,
                document_features.catalog_contains_lang,
                limits,
                plan.annotations,
            )?;
            let object_limits = syntax.object_limits.clone();
            let stream_safety = crate::stream_safety::inspect(&document, limits, bytes, &syntax)?;
            let unicode_names = crate::unicode_names::inspect(&document, &pages, limits)?;
            let font_embedding =
                font_embedding::inspect(&document, &content, limits, plan.font_details)?;
            InspectionSummary {
                header,
                content,
                font_embedding,
                icc_based,
                xobjects,
                graphics,
                annotations,
                actions,
                forms,
                document_features,
                object_limits,
                stream_safety,
                unicode_names,
            }
        };
        Ok((normalized, inspections))
    }
}

/// Returns the raw `/P` permission bits while retaining access to them after
/// lopdf authenticates an encrypted document and removes its encryption
/// dictionary from the normalized object graph.
fn encryption_permissions(document: &Document) -> (Option<u64>, Option<PdfObjectId>) {
    let encryption_dictionary_object = document
        .trailer
        .get(b"Encrypt")
        .ok()
        .and_then(|object| object.as_reference().ok())
        .map(PdfObjectId::from)
        .or_else(|| {
            document
                .encryption_state
                .as_ref()
                .and_then(|state| state.encrypt_object_id())
                .map(PdfObjectId::from)
        });
    let permissions = document
        .get_encrypted()
        .ok()
        .and_then(|dictionary| dictionary.get(b"P").ok())
        .and_then(|object| object.as_i64().ok())
        .map(i64::cast_unsigned)
        .or_else(|| {
            document
                .encryption_state
                .as_ref()
                .map(|state| state.permissions().bits())
        });
    (permissions, encryption_dictionary_object)
}

/// Returns whether the original PDF was encrypted and whether its contents
/// are still unavailable. `lopdf` removes `/Encrypt` after successfully
/// authenticating with the empty password, while `was_encrypted()` remains
/// true so callers can still report the PDF/A conformance failure.
fn encryption_status(document: &Document) -> (bool, bool) {
    let encrypted_content_unavailable = contains_key(&document.trailer, b"Encrypt");
    (
        document.was_encrypted() || encrypted_content_unavailable,
        encrypted_content_unavailable,
    )
}

fn extract_trailer_id(document: &Document) -> Option<Vec<Vec<u8>>> {
    let Object::Array(values) = document.trailer.get(b"ID").ok()? else {
        return None;
    };
    (values.len() == 2).then_some(())?;
    values
        .iter()
        .map(|value| match value {
            Object::String(bytes, _) => Some(bytes.clone()),
            _ => None,
        })
        .collect()
}

fn load_document(bytes: &[u8], limits: &SafetyLimits) -> Result<Document, PdfError> {
    if bytes.len() as u64 > limits.max_input_size {
        return Err(PdfError::InputTooLarge {
            actual: bytes.len() as u64,
            limit: limits.max_input_size,
        });
    }

    let options = LoadOptions {
        strict: true,
        max_decompressed_size: Some(limits.max_decoded_stream_size),
        ..LoadOptions::default()
    };
    let document = if let Some(repaired) = crate::syntax::repair_for_lopdf(bytes) {
        Document::load_mem_with_options(&repaired, options.clone())
            .or_else(|_| Document::load_mem_with_options(bytes, options))?
    } else {
        Document::load_mem_with_options(bytes, options)?
    };
    Ok(document)
}

fn enforce_object_limit(document: &Document, limits: &SafetyLimits) -> Result<(), PdfError> {
    let indirect_count = document
        .reference_table
        .entries
        .values()
        .filter(|entry| {
            !matches!(
                entry,
                lopdf::xref::XrefEntry::Free | lopdf::xref::XrefEntry::UnusableFree
            )
        })
        .count();
    if indirect_count > SafetyLimits::PDF_A1_MAX_INDIRECT_OBJECTS {
        return Err(PdfError::TooManyIndirectObjects {
            actual: indirect_count,
            limit: SafetyLimits::PDF_A1_MAX_INDIRECT_OBJECTS,
        });
    }
    let actual = document.objects.len();
    if actual > limits.max_object_count {
        return Err(PdfError::TooManyObjects {
            actual,
            limit: limits.max_object_count,
        });
    }
    Ok(())
}

fn inspect_catalog_metadata(
    document: &Document,
    catalog: Option<&Dictionary>,
    limits: &SafetyLimits,
) -> Result<CatalogMetadataStream, PdfError> {
    let Some(catalog) = catalog else {
        return Ok(CatalogMetadataStream::default());
    };
    let Ok(entry) = catalog.get(b"Metadata") else {
        return Ok(CatalogMetadataStream::default());
    };
    let mut result = CatalogMetadataStream {
        present: true,
        ..CatalogMetadataStream::default()
    };
    let Some(object) = resolve_optional(document, entry, limits.max_reference_depth)? else {
        return Ok(result);
    };
    let Ok(stream) = object.as_stream() else {
        return Ok(result);
    };
    result.is_stream = true;
    result.type_is_metadata = stream.dict.get_type().ok() == Some(b"Metadata".as_slice());
    result.subtype_is_xml = stream
        .dict
        .get(b"Subtype")
        .ok()
        .and_then(|object| object.as_name().ok())
        == Some(b"XML".as_slice());
    result.has_filter = contains_key(&stream.dict, b"Filter");
    Ok(result)
}

fn reference_id(object: &Object) -> Option<PdfObjectId> {
    object.as_reference().ok().map(Into::into)
}

fn extract_info(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<(DocumentMetadata, Option<PdfObjectId>), PdfError> {
    let Ok(info_entry) = document.trailer.get(b"Info") else {
        return Ok((DocumentMetadata::default(), None));
    };
    let object_id = reference_id(info_entry);
    let info = resolve(document, info_entry, limits.max_reference_depth)?
        .as_dict()
        .map_err(|error| {
            let _ = error;
            PdfError::UnexpectedObject("Info is not a dictionary")
        })?;

    let mut values = BTreeMap::new();
    for key in [
        b"Title".as_slice(),
        b"Author",
        b"Subject",
        b"Keywords",
        b"Creator",
        b"Producer",
        b"CreationDate",
        b"ModDate",
        b"Trapped",
    ] {
        if let Ok(value) = info.get(key)
            && let Ok(value) = resolve(document, value, limits.max_reference_depth)
            && let Some(text) = object_text(
                value,
                !matches!(key, b"CreationDate" | b"ModDate" | b"Trapped"),
            )
        {
            values.insert(String::from_utf8_lossy(key).into_owned(), text);
        }
    }

    Ok((DocumentMetadata { values }, object_id))
}

fn object_text(object: &Object, stringify_non_string: bool) -> Option<String> {
    match object {
        Object::String(bytes, _) => Some(decode_verapdf_pdf_string(bytes)).map(|mut value| {
            if stringify_non_string && value.ends_with('\0') {
                value.pop();
            }
            value
        }),
        Object::Null => None,
        _ if stringify_non_string => Some(verapdf_object_string(object)),
        _ => None,
    }
}

fn verapdf_object_string(object: &Object) -> String {
    match object {
        Object::Null => "null".to_owned(),
        // COSBoolean inherits Object.toString(), so its value is an unstable
        // identity string and can never be a portable XMP text equivalent.
        Object::Boolean(_) => "org.verapdf.cos.COSBoolean@<identity>".to_owned(),
        Object::Integer(value) => value.to_string(),
        Object::Real(value) => {
            let mut value = format!("{value:.6}");
            while value.ends_with('0') && !value.ends_with(".0") {
                value.pop();
            }
            value
        }
        Object::Name(name) => format!("/{}", String::from_utf8_lossy(name)),
        Object::String(bytes, _) => decode_verapdf_pdf_string(bytes),
        Object::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(verapdf_object_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Object::Dictionary(dictionary) => format!("dictionary(size = {})", dictionary.len()),
        Object::Stream(stream) => format!("stream(size = {})", stream.content.len()),
        Object::Reference((object, generation)) => format!("{object} {generation} R"),
    }
}

pub(crate) fn decode_verapdf_pdf_string(bytes: &[u8]) -> String {
    if let Some(bytes) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let mut units = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes(*pair))
            .collect::<Vec<_>>();
        if !bytes.len().is_multiple_of(2) {
            units.push(0xFFFD);
        }
        return String::from_utf16_lossy(&units);
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    bytes
        .iter()
        .map(|byte| char::from_u32(verapdf_pdfdoc_codepoint(*byte)).expect("valid code point"))
        .collect()
}

fn verapdf_pdfdoc_codepoint(byte: u8) -> u32 {
    match byte {
        24 => 728,
        25 => 711,
        26 => 710,
        27 => 729,
        28 => 733,
        29 => 731,
        30 => 730,
        31 => 732,
        127 | 159 => 0xFFFD,
        128 => 8226,
        129 => 8224,
        130 => 8225,
        131 => 8230,
        132 => 8212,
        133 => 8211,
        134 => 402,
        135 => 8260,
        136 => 8249,
        137 => 8250,
        138 => 8722,
        139 => 8240,
        140 => 8222,
        141 => 8220,
        142 => 8221,
        143 => 8216,
        144 => 8217,
        145 => 8218,
        146 => 8482,
        147 => 64257,
        148 => 64258,
        149 => 321,
        150 => 338,
        151 => 352,
        152 => 376,
        153 => 381,
        154 => 305,
        155 => 322,
        156 => 339,
        157 => 353,
        158 => 382,
        160 => 8364,
        _ => u32::from(byte),
    }
}

struct XmpExtraction {
    xmp: Option<XmpMetadata>,
    object_id: Option<PdfObjectId>,
    parse_error: Option<String>,
}

fn extract_xmp(
    document: &Document,
    catalog: Option<&Dictionary>,
    limits: &SafetyLimits,
) -> Result<XmpExtraction, PdfError> {
    let none = || XmpExtraction {
        xmp: None,
        object_id: None,
        parse_error: None,
    };
    let Some(catalog) = catalog else {
        return Ok(none());
    };
    let Ok(metadata_entry) = catalog.get(b"Metadata") else {
        return Ok(none());
    };
    let object_id = reference_id(metadata_entry);
    let resolved = resolve_optional(document, metadata_entry, limits.max_reference_depth)?.ok_or(
        PdfError::UnexpectedObject("Metadata reference does not resolve to an object"),
    );
    let stream = match resolved.and_then(|object| {
        object.as_stream().map_err(|error| {
            let _ = error;
            PdfError::UnexpectedObject("Metadata is not a stream")
        })
    }) {
        Ok(stream) => stream,
        Err(error) => {
            return Ok(XmpExtraction {
                xmp: None,
                object_id,
                parse_error: Some(error.to_string()),
            });
        }
    };
    let bytes = match stream.decompressed_content_with_limit(limits.max_decoded_stream_size) {
        Ok(bytes) => bytes,
        Err(
            error @ lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. }),
        ) => return Err(PdfError::XmpDecodeLimit(error.to_string())),
        Err(error) => {
            return Ok(XmpExtraction {
                xmp: None,
                object_id,
                parse_error: Some(format!("could not decode XMP stream: {error}")),
            });
        }
    };
    Ok(match parse_xmp(&bytes) {
        Ok(xmp) => XmpExtraction {
            xmp: Some(xmp),
            object_id,
            parse_error: None,
        },
        Err(error) => XmpExtraction {
            xmp: None,
            object_id,
            parse_error: Some(error),
        },
    })
}

fn extract_output_intents(
    document: &Document,
    catalog: Option<&Dictionary>,
    limits: &SafetyLimits,
) -> Result<OutputIntentsSummary, PdfError> {
    let Some(catalog) = catalog else {
        return Ok(OutputIntentsSummary::default());
    };
    let Ok(entry) = catalog.get(b"OutputIntents") else {
        return Ok(OutputIntentsSummary::default());
    };
    let mut summary = OutputIntentsSummary {
        present: true,
        ..OutputIntentsSummary::default()
    };
    let Some(resolved) = resolve_optional(document, entry, limits.max_reference_depth)? else {
        return Ok(summary);
    };
    let Ok(array) = resolved.as_array() else {
        return Ok(summary);
    };
    summary.is_array = true;
    for item in array {
        summary
            .entries
            .push(inspect_output_intent(document, item, limits)?);
    }
    Ok(summary)
}

fn inspect_output_intent(
    document: &Document,
    item: &Object,
    limits: &SafetyLimits,
) -> Result<OutputIntentSummary, PdfError> {
    let mut summary = OutputIntentSummary {
        object_id: reference_id(item),
        ..OutputIntentSummary::default()
    };
    let Some(resolved) = resolve_optional(document, item, limits.max_reference_depth)? else {
        return Ok(summary);
    };
    let Some(dictionary) = dictionary_based(resolved) else {
        return Ok(summary);
    };
    summary.is_dictionary_based = true;

    if let Ok(subtype) = dictionary.get(b"S") {
        summary.subtype_present = true;
        let resolved =
            resolve_optional(document, subtype, limits.max_reference_depth)?.unwrap_or(subtype);
        summary.subtype = resolved.as_name().ok().map(signature);
    }

    summary.dest_output_profile_ref_present = contains_key(dictionary, b"DestOutputProfileRef");

    let Ok(profile) = dictionary.get(b"DestOutputProfile") else {
        return Ok(summary);
    };
    summary.dest_output_profile_present = true;
    summary.dest_output_profile_id = reference_id(profile);
    let Some(resolved) = resolve_optional(document, profile, limits.max_reference_depth)? else {
        return Ok(summary);
    };
    let Ok(stream) = resolved.as_stream() else {
        return Ok(summary);
    };
    summary.dest_output_profile_is_stream = true;
    match stream.decompressed_content_with_limit(limits.max_decoded_stream_size) {
        Ok(bytes) => summary.dest_output_profile_header = IccHeader::parse(&bytes),
        Err(
            error @ lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. }),
        ) => return Err(PdfError::IccDecodeLimit(error.to_string())),
        Err(error) => match decode_ascii_hex_stream(stream, limits.max_decoded_stream_size) {
            Ok(Some(bytes)) => summary.dest_output_profile_header = IccHeader::parse(&bytes),
            Ok(None) => {
                summary.dest_output_profile_decode_error = Some(format!(
                    "could not decode ICC output profile stream: {error}"
                ));
            }
            Err(message) => {
                summary.dest_output_profile_decode_error = Some(format!(
                    "could not decode ICC output profile stream: {message}"
                ));
            }
        },
    }
    Ok(summary)
}

fn decode_ascii_hex_stream(
    stream: &lopdf::Stream,
    max_decoded_size: usize,
) -> Result<Option<Vec<u8>>, String> {
    let is_ascii_hex = stream
        .dict
        .get(b"Filter")
        .ok()
        .and_then(|filter| filter.as_name().ok())
        .is_some_and(|filter| filter == b"ASCIIHexDecode");
    if !is_ascii_hex {
        return Ok(None);
    }
    let mut decoded = Vec::with_capacity(stream.content.len() / 2);
    let mut high_nibble = None;
    for byte in &stream.content {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if *byte == b'>' {
            break;
        }
        let nibble = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return Err("invalid ASCII hexadecimal stream".to_owned()),
        };
        if let Some(high) = high_nibble.take() {
            if decoded.len() >= max_decoded_size {
                return Err(format!(
                    "decoded stream exceeds configured limit of {max_decoded_size} bytes"
                ));
            }
            decoded.push((high << 4) | nibble);
        } else {
            high_nibble = Some(nibble);
        }
    }
    if let Some(high) = high_nibble {
        if decoded.len() >= max_decoded_size {
            return Err(format!(
                "decoded stream exceeds configured limit of {max_decoded_size} bytes"
            ));
        }
        decoded.push(high << 4);
    }
    Ok(Some(decoded))
}

fn signature(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

fn summarize_fonts(document: &Document, limits: &SafetyLimits) -> Result<FontSummary, PdfError> {
    let mut summary = FontSummary::default();
    for object in document.objects.values() {
        let dictionary = match object {
            Object::Dictionary(dictionary) => dictionary,
            Object::Stream(stream) => &stream.dict,
            _ => continue,
        };
        if dictionary.get_type().ok() != Some(b"Font".as_slice()) {
            continue;
        }
        summary.total += 1;
        if font_is_embedded(document, dictionary, limits)? {
            summary.embedded += 1;
        }
    }
    Ok(summary)
}

/// A coarse, whole-document proxy: counts every `/Type /Font` object
/// regardless of use, and treats a present `/FontFile*` key as "embedded"
/// without validating the program bytes. This intentionally differs from
/// the pinned `PDFA1B-FONT-EMBEDDING-001` predicate in `font_embedding.rs`,
/// which is bounded to fonts reached via text-show content paths and
/// requires a recognized font program (`valid_font_program`). The two must
/// not be unified: they cover different populations (every font object vs.
/// only used/reached ones).
fn font_is_embedded(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Ok(descriptor_entry) = font.get(b"FontDescriptor") else {
        return Ok(false);
    };
    let Some(descriptor) =
        resolve_optional(document, descriptor_entry, limits.max_reference_depth)?
            .and_then(|object| object.as_dict().ok())
    else {
        return Ok(false);
    };
    Ok(descriptor.has(b"FontFile") || descriptor.has(b"FontFile2") || descriptor.has(b"FontFile3"))
}

#[cfg(test)]
mod tests {
    use lopdf::{Object, StringFormat, dictionary};

    use super::*;

    /// Confirmed against veraPDF 1.30.2: a trailer with a direct `/Encrypt
    /// null` is compliant (not encrypted), matching the same direct-null
    /// convention as every other `containsX` predicate. Built directly on
    /// an in-memory `Document` rather than round-tripped through bytes:
    /// `lopdf` itself special-cases *any* trailer `/Encrypt` key (even a
    /// literal null) at load time and does not populate `document.objects`
    /// for such a file — a separate, upstream parsing quirk unrelated to
    /// this flag-computation fix, and out of scope to work around here (a
    /// literal-null `/Encrypt` trailer entry is not a realistic PDF
    /// producer's output).
    #[test]
    fn direct_null_encrypt_key_is_not_encrypted() {
        let mut document = Document::with_version("1.4");
        document.trailer.set("Encrypt", Object::Null);
        let catalog_id = document.add_object(dictionary! { "Type" => "Catalog" });
        document.trailer.set("Root", catalog_id);

        let normalized =
            PdfDocument::normalize(&document, &SafetyLimits::default(), None).expect("normalize");
        assert!(!normalized.encrypted);
    }

    #[test]
    fn encrypted_documents_are_normalized_before_the_object_safety_cap() {
        let limits = SafetyLimits {
            max_object_count: 0,
            ..SafetyLimits::default()
        };
        let document =
            PdfDocument::from_bytes(include_bytes!("../tests/fixtures/encrypted.pdf"), &limits)
                .expect("encryption is a terminal PDF/A finding");
        assert!(document.encrypted);
    }

    #[test]
    fn extracts_info_dictionary() {
        let mut document = Document::with_version("1.4");
        let info_id = document.add_object(dictionary! {
            "Title" => Object::String(b"Example".to_vec(), StringFormat::Literal),
            "Author" => Object::String(b"Ferris".to_vec(), StringFormat::Literal),
        });
        document.trailer.set("Info", info_id);
        let (metadata, id) = extract_info(&document, &SafetyLimits::default()).expect("metadata");
        assert_eq!(id, Some(info_id.into()));
        assert_eq!(metadata.values["Title"], "Example");
        assert_eq!(metadata.values["Author"], "Ferris");
    }

    #[test]
    fn decodes_info_strings_using_pdfdoc_encoding() {
        let mut document = Document::with_version("1.4");
        let info_id = document.add_object(dictionary! {
            "Title" => Object::String(vec![b't', b'e', b'x', b't', 0x8B], StringFormat::Literal),
        });
        document.trailer.set("Info", info_id);

        let (metadata, _) = extract_info(&document, &SafetyLimits::default()).expect("metadata");

        assert_eq!(metadata.values["Title"], "text‰");
    }

    #[test]
    fn stringifies_non_string_info_values_like_verapdf() {
        assert_eq!(verapdf_object_string(&Object::Integer(42)), "42");
        assert_eq!(verapdf_object_string(&Object::Real(3.0)), "3.0");
        assert_eq!(verapdf_object_string(&Object::Real(3.125)), "3.125");
        assert_eq!(
            verapdf_object_string(&Object::Name(b"Foo".to_vec())),
            "/Foo"
        );
        assert_eq!(
            verapdf_object_string(&Object::Array(vec![
                Object::Integer(1),
                Object::Name(b"Foo".to_vec()),
            ])),
            "[1, /Foo]"
        );
        assert_eq!(
            verapdf_object_string(&Object::Dictionary(dictionary! { "A" => 1 })),
            "dictionary(size = 1)"
        );
    }

    #[test]
    fn resolves_indirect_info_values_before_stringification() {
        let mut document = Document::with_version("1.4");
        let title = document.add_object(Object::Integer(42));
        let info_id = document.add_object(dictionary! { "Title" => title });
        document.trailer.set("Info", info_id);
        let (metadata, _) = extract_info(&document, &SafetyLimits::default()).expect("metadata");
        assert_eq!(metadata.values["Title"], "42");
    }

    #[test]
    fn accepts_only_two_string_trailer_ids() {
        let mut document = Document::with_version("1.4");
        document
            .trailer
            .set("ID", vec![Object::string_literal("one")]);
        assert_eq!(extract_trailer_id(&document), None);
        document.trailer.set(
            "ID",
            vec![Object::string_literal("one"), Object::string_literal("two")],
        );
        assert_eq!(
            extract_trailer_id(&document),
            Some(vec![b"one".to_vec(), b"two".to_vec()])
        );
    }

    #[test]
    fn inspection_plan_requires_complete_content_discovery() {
        let plan = InspectionPlan::all();
        assert_eq!(plan.font_details, InspectionNeed::Unknown);
        assert_eq!(
            plan.after_content_discovery(false, false, false, false, false)
                .font_details,
            InspectionNeed::NotApplicable
        );
        assert_eq!(
            plan.after_content_discovery(true, false, false, false, false)
                .font_details,
            InspectionNeed::Required
        );
        assert_eq!(
            plan.after_content_discovery(false, true, false, false, false)
                .xobjects,
            InspectionNeed::Required
        );
        let absent = plan.after_content_discovery(false, false, false, false, false);
        assert_eq!(absent.annotations, InspectionNeed::NotApplicable);
        assert_eq!(absent.forms, InspectionNeed::NotApplicable);
        assert_eq!(absent.actions, InspectionNeed::NotApplicable);
        let present = plan.after_content_discovery(false, false, true, true, true);
        assert_eq!(present.annotations, InspectionNeed::Required);
        assert_eq!(present.forms, InspectionNeed::Required);
        assert_eq!(present.actions, InspectionNeed::Required);
    }

    #[test]
    fn unknown_inspection_needs_run_conservatively() {
        assert!(InspectionNeed::Unknown.should_run());
        assert!(InspectionNeed::Required.should_run());
        assert!(!InspectionNeed::NotApplicable.should_run());
    }
}
