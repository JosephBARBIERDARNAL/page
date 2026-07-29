use std::collections::{BTreeMap, BTreeSet};

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::content_support::{decode_content_stream, inherited_page_resources, resource_once};
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{ResourceKey, resolve_optional};
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct FontEmbeddingSummary {
    pub(crate) failures: Vec<RuleFailure>,
    pub(crate) invalid_types: Vec<RuleFailure>,
    pub(crate) invalid_subtypes: Vec<RuleFailure>,
    pub(crate) invalid_base_fonts: Vec<RuleFailure>,
    pub(crate) invalid_first_chars: Vec<RuleFailure>,
    pub(crate) invalid_last_chars: Vec<RuleFailure>,
    pub(crate) invalid_widths: Vec<RuleFailure>,
    pub(crate) invalid_font_file_subtypes: Vec<RuleFailure>,
    pub(crate) incompatible_type0_system_info: Vec<RuleFailure>,
    pub(crate) invalid_cid_to_gid_maps: Vec<RuleFailure>,
    pub(crate) unembedded_cmaps: Vec<RuleFailure>,
    pub(crate) invalid_cmap_wmodes: Vec<RuleFailure>,
    pub(crate) invalid_cmap_cids: Vec<RuleFailure>,
    pub(crate) invalid_type1_subset_charsets: Vec<RuleFailure>,
    pub(crate) invalid_cid_subset_cidsets: Vec<RuleFailure>,
    pub(crate) invalid_nonsymbolic_truetype_encodings: Vec<RuleFailure>,
    pub(crate) invalid_symbolic_truetype_encodings: Vec<RuleFailure>,
    pub(crate) invalid_symbolic_truetype_cmaps: Vec<RuleFailure>,
    pub(crate) excessive_graphics_state_nesting: Vec<RuleFailure>,
}

type CidSystemInfo = (Vec<u8>, Vec<u8>);

#[derive(Clone)]
struct SelectedFont {
    key: ResourceKey,
    object: Object,
    description: String,
}

#[derive(Clone, Default)]
struct GraphicsState {
    font: Option<SelectedFont>,
    rendering_mode: i64,
}

#[derive(Default)]
struct FontUse {
    object_id: Option<PdfObjectId>,
    description: String,
    subtype: Option<String>,
    embedded: bool,
    visible: bool,
}

struct Scanner<'a> {
    document: &'a Document,
    limits: &'a SafetyLimits,
    uses: BTreeMap<ResourceKey, FontUse>,
    active_descendant_fonts: BTreeSet<ResourceKey>,
    invalid_types: Vec<RuleFailure>,
    invalid_subtypes: Vec<RuleFailure>,
    invalid_base_fonts: Vec<RuleFailure>,
    invalid_first_chars: Vec<RuleFailure>,
    invalid_last_chars: Vec<RuleFailure>,
    invalid_widths: Vec<RuleFailure>,
    invalid_font_file_subtypes: Vec<RuleFailure>,
    incompatible_type0_system_info: Vec<RuleFailure>,
    invalid_cid_to_gid_maps: Vec<RuleFailure>,
    unembedded_cmaps: Vec<RuleFailure>,
    invalid_cmap_wmodes: Vec<RuleFailure>,
    invalid_cmap_cids: Vec<RuleFailure>,
    invalid_type1_subset_charsets: Vec<RuleFailure>,
    invalid_cid_subset_cidsets: Vec<RuleFailure>,
    invalid_nonsymbolic_truetype_encodings: Vec<RuleFailure>,
    invalid_symbolic_truetype_encodings: Vec<RuleFailure>,
    invalid_symbolic_truetype_cmaps: Vec<RuleFailure>,
    excessive_graphics_state_nesting: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<FontEmbeddingSummary, PdfError> {
    let mut scanner = Scanner {
        document,
        limits,
        uses: BTreeMap::new(),
        active_descendant_fonts: BTreeSet::new(),
        invalid_types: Vec::new(),
        invalid_subtypes: Vec::new(),
        invalid_base_fonts: Vec::new(),
        invalid_first_chars: Vec::new(),
        invalid_last_chars: Vec::new(),
        invalid_widths: Vec::new(),
        invalid_font_file_subtypes: Vec::new(),
        incompatible_type0_system_info: Vec::new(),
        invalid_cid_to_gid_maps: Vec::new(),
        unembedded_cmaps: Vec::new(),
        invalid_cmap_wmodes: Vec::new(),
        invalid_cmap_cids: Vec::new(),
        invalid_type1_subset_charsets: Vec::new(),
        invalid_cid_subset_cidsets: Vec::new(),
        invalid_nonsymbolic_truetype_encodings: Vec::new(),
        invalid_symbolic_truetype_encodings: Vec::new(),
        invalid_symbolic_truetype_cmaps: Vec::new(),
        excessive_graphics_state_nesting: Vec::new(),
    };
    for (page_number, page_id) in document.get_pages() {
        scanner.scan_page(page_number, page_id)?;
    }
    scanner
        .invalid_cmap_cids
        .extend(inspect_all_embedded_cmap_cids(document, limits)?);
    scanner.invalid_cmap_cids.sort_by(|left, right| {
        left.object_id
            .cmp(&right.object_id)
            .then_with(|| left.description.cmp(&right.description))
    });
    scanner.invalid_cmap_cids.dedup_by(|left, right| {
        left.object_id == right.object_id && left.description == right.description
    });

    let failures = scanner
        .uses
        .into_values()
        .filter(|usage| {
            usage.visible
                && !usage.embedded
                && !matches!(usage.subtype.as_deref(), Some("Type3" | "Type0"))
        })
        .map(|usage| RuleFailure {
            object_id: usage.object_id,
            description: usage.description,
        })
        .collect();
    Ok(FontEmbeddingSummary {
        failures,
        invalid_types: scanner.invalid_types,
        invalid_subtypes: scanner.invalid_subtypes,
        invalid_base_fonts: scanner.invalid_base_fonts,
        invalid_first_chars: scanner.invalid_first_chars,
        invalid_last_chars: scanner.invalid_last_chars,
        invalid_widths: scanner.invalid_widths,
        invalid_font_file_subtypes: scanner.invalid_font_file_subtypes,
        incompatible_type0_system_info: scanner.incompatible_type0_system_info,
        invalid_cid_to_gid_maps: scanner.invalid_cid_to_gid_maps,
        unembedded_cmaps: scanner.unembedded_cmaps,
        invalid_cmap_wmodes: scanner.invalid_cmap_wmodes,
        invalid_cmap_cids: scanner.invalid_cmap_cids,
        invalid_type1_subset_charsets: scanner.invalid_type1_subset_charsets,
        invalid_cid_subset_cidsets: scanner.invalid_cid_subset_cidsets,
        invalid_nonsymbolic_truetype_encodings: scanner.invalid_nonsymbolic_truetype_encodings,
        invalid_symbolic_truetype_encodings: scanner.invalid_symbolic_truetype_encodings,
        invalid_symbolic_truetype_cmaps: scanner.invalid_symbolic_truetype_cmaps,
        excessive_graphics_state_nesting: scanner.excessive_graphics_state_nesting,
    })
}

fn inspect_all_embedded_cmap_cids(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<Vec<RuleFailure>, PdfError> {
    let mut failures = Vec::new();
    for (object_id, object) in &document.objects {
        let Ok(stream) = object.as_stream() else {
            continue;
        };
        if !stream.dict.has(b"CMapName") {
            continue;
        }
        let bytes = decode_font_stream(stream, limits)?;
        if cmap_maximal_cid(&bytes).is_some_and(|cid| cid > 65_535) {
            failures.push(font_failure(
                Some((*object_id).into()),
                &format!("embedded CMap stream {} {}", object_id.0, object_id.1),
                "contains a CID greater than 65,535",
            ));
        }
    }
    Ok(failures)
}

impl Scanner<'_> {
    fn scan_page(&mut self, page_number: u32, page_id: ObjectId) -> Result<(), PdfError> {
        let page = self
            .document
            .objects
            .get(&page_id)
            .and_then(|object| object.as_dict().ok())
            .ok_or(PdfError::UnexpectedObject("page is not a dictionary"))?;
        let resources = inherited_page_resources(self.document, page, self.limits)?;
        let mut state = GraphicsState::default();
        let mut stack = Vec::new();
        let mut active_forms = BTreeSet::new();
        let mut decoded_bytes = 0usize;
        if let Ok(contents) = page.get(b"Contents") {
            self.scan_contents(
                contents,
                resources,
                &mut state,
                &mut stack,
                &mut active_forms,
                &mut decoded_bytes,
                &format!("page {page_number}"),
                0,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_contents(
        &mut self,
        contents: &Object,
        resources: Option<&Dictionary>,
        state: &mut GraphicsState,
        stack: &mut Vec<GraphicsState>,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let Some(contents) =
            resolve_optional(self.document, contents, self.limits.max_reference_depth)?
        else {
            return Ok(());
        };
        if let Ok(array) = contents.as_array() {
            for item in array {
                self.scan_contents(
                    item,
                    resources,
                    state,
                    stack,
                    active_forms,
                    decoded_bytes,
                    context,
                    depth + 1,
                )?;
            }
            return Ok(());
        }
        let Ok(stream) = contents.as_stream() else {
            return Ok(());
        };
        let bytes = decode_content_stream(stream, self.limits, decoded_bytes)?;
        let content_bytes = crate::icc_based::content_without_inline_images(&bytes);
        let Ok(content) = Content::decode(&content_bytes) else {
            return Ok(());
        };
        for operation in content.operations {
            match operation.operator.as_str() {
                "q" => {
                    if stack.len() >= self.limits.max_reference_depth {
                        return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
                    }
                    stack.push(state.clone());
                    if stack.len() > 28 {
                        self.excessive_graphics_state_nesting.push(RuleFailure {
                            object_id: None,
                            description: format!(
                                "{context} reaches graphics-state nesting depth {}",
                                stack.len()
                            ),
                        });
                    }
                }
                "Q" => {
                    if let Some(saved) = stack.pop() {
                        *state = saved;
                    }
                }
                "Tf" => {
                    let name = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok());
                    state.font = match name {
                        Some(name) => {
                            resource_once(self.document, self.limits, resources, b"Font", name)?
                                .map(|object| SelectedFont {
                                    key: object_key(object, context, operation.operands.first()),
                                    object: object.clone(),
                                    description: describe_font(
                                        object,
                                        context,
                                        operation.operands.first(),
                                    ),
                                })
                        }
                        None => None,
                    };
                }
                "Tr" => {
                    if let Some(mode) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_i64().ok())
                    {
                        state.rendering_mode = mode;
                    }
                }
                "Tj" | "TJ" | "'" | "\"" if shows_text(&operation.operands) => {
                    if let Some(font) = state.font.clone() {
                        self.record_font(&font, state.rendering_mode)?;
                    }
                }
                "Do" => {
                    let Some(name) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok())
                    else {
                        continue;
                    };
                    let Some(object) =
                        resource_once(self.document, self.limits, resources, b"XObject", name)?
                    else {
                        continue;
                    };
                    let form_id = object.as_reference().ok();
                    if form_id.is_some_and(|id| !active_forms.insert(id)) {
                        continue;
                    }
                    let result = self.scan_form(
                        object,
                        resources,
                        state,
                        active_forms,
                        decoded_bytes,
                        &format!("{context}/Form /{}", String::from_utf8_lossy(name)),
                        depth + 1,
                    );
                    if let Some(id) = form_id {
                        active_forms.remove(&id);
                    }
                    result?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_form(
        &mut self,
        object: &Object,
        parent_resources: Option<&Dictionary>,
        state: &GraphicsState,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        let Some(form) = resolve_optional(self.document, object, self.limits.max_reference_depth)?
            .and_then(|object| object.as_stream().ok())
        else {
            return Ok(());
        };
        if form
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
            != Some(b"Form".as_slice())
        {
            return Ok(());
        }
        let resources = match form.dict.get(b"Resources") {
            Ok(entry) => resolve_optional(self.document, entry, self.limits.max_reference_depth)?
                .and_then(|object| object.as_dict().ok()),
            Err(_) => parent_resources,
        };
        let mut form_state = state.clone();
        let mut stack = Vec::new();
        self.scan_contents(
            object,
            resources,
            &mut form_state,
            &mut stack,
            active_forms,
            decoded_bytes,
            context,
            depth,
        )
    }

    fn record_font(
        &mut self,
        selected: &SelectedFont,
        rendering_mode: i64,
    ) -> Result<(), PdfError> {
        if self.uses.contains_key(&selected.key) {
            return Ok(());
        }
        let Some(object) = resolve_optional(
            self.document,
            &selected.object,
            self.limits.max_reference_depth,
        )?
        else {
            return Ok(());
        };
        let Ok(font) = object.as_dict() else {
            return Ok(());
        };
        let subtype = font
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
            .map(|value| String::from_utf8_lossy(value).into_owned());
        // veraPDF 1.28.2 does not create a PDFont model object for a missing
        // or unsupported subtype, so none of the PDFont predicates are
        // instantiated for such a resource.
        if !matches!(
            subtype.as_deref(),
            Some(
                "Type1"
                    | "MMType1"
                    | "TrueType"
                    | "Type3"
                    | "Type0"
                    | "CIDFontType0"
                    | "CIDFontType2"
            )
        ) {
            return Ok(());
        }
        let object_id = selected.key.object_id();
        self.inspect_font_dictionary(font, object_id, &selected.description, subtype.as_deref())?;
        if subtype.as_deref() == Some("Type0") {
            self.inspect_composite_font(font, object_id, &selected.description, rendering_mode)?;
        } else if subtype.as_deref() == Some("TrueType") {
            self.inspect_truetype_font(font, object_id, &selected.description)?;
        } else if subtype.as_deref() == Some("Type1") {
            self.inspect_type1_subset_font(font, object_id, &selected.description)?;
        }
        let embedded =
            if rendering_mode == 3 || matches!(subtype.as_deref(), Some("Type3" | "Type0")) {
                false
            } else {
                font_is_embedded(self.document, font, self.limits)?
            };
        self.uses.entry(selected.key.clone()).or_insert(FontUse {
            object_id,
            description: selected.description.clone(),
            subtype: subtype.clone(),
            embedded,
            // veraPDF 1.28.2 associates the first observed rendering mode
            // with a font model object and does not revise it on later uses.
            visible: rendering_mode != 3,
        });

        if subtype.as_deref() == Some("Type0")
            && self.active_descendant_fonts.insert(selected.key.clone())
        {
            if let Ok(descendants) = font.get(b"DescendantFonts")
                && let Some(descendants) =
                    resolve_optional(self.document, descendants, self.limits.max_reference_depth)?
                        .and_then(|object| object.as_array().ok())
            {
                for (index, descendant) in descendants.iter().enumerate() {
                    let context = format!("{}/descendant {index}", selected.description);
                    let descendant = SelectedFont {
                        key: object_key(descendant, &context, None),
                        object: descendant.clone(),
                        description: describe_descendant(descendant, &selected.description, index),
                    };
                    self.record_font(&descendant, rendering_mode)?;
                }
            }
            self.active_descendant_fonts.remove(&selected.key);
        }
        Ok(())
    }

    fn inspect_truetype_font(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
    ) -> Result<(), PdfError> {
        let descriptor = font_descriptor_dictionary(self.document, font, self.limits)?;
        let symbolic = descriptor
            .and_then(|descriptor| descriptor.get(b"Flags").ok())
            .and_then(as_integer)
            .is_some_and(|flags| flags & 4 != 0);
        let (encoding, contains_differences) = truetype_encoding(self.document, font, self.limits)?;

        if symbolic {
            if encoding.is_some() {
                self.invalid_symbolic_truetype_encodings.push(font_failure(
                    object_id,
                    description,
                    "is symbolic but specifies an /Encoding",
                ));
            }
            if let Some(cmap_count) = truetype_cmap_count(self.document, descriptor, self.limits)?
                && cmap_count != 1
            {
                self.invalid_symbolic_truetype_cmaps.push(font_failure(
                    object_id,
                    description,
                    &format!(
                        "is symbolic but its embedded TrueType program contains {cmap_count} cmap subtables"
                    ),
                ));
            }
        } else if !matches!(
            encoding.as_deref(),
            Some(b"MacRomanEncoding" | b"WinAnsiEncoding")
        ) || contains_differences
        {
            self.invalid_nonsymbolic_truetype_encodings
                .push(font_failure(
                    object_id,
                    description,
                    "is non-symbolic but lacks an unmodified MacRomanEncoding or WinAnsiEncoding",
                ));
        }
        Ok(())
    }

    fn inspect_type1_subset_font(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
    ) -> Result<(), PdfError> {
        let is_subset = font
            .get(b"BaseFont")
            .ok()
            .and_then(|value| value.as_name().ok())
            .is_some_and(is_subset_font_name);
        if !is_subset {
            return Ok(());
        }
        let has_charset = font_descriptor_dictionary(self.document, font, self.limits)?
            .and_then(|descriptor| descriptor.get(b"CharSet").ok())
            .is_some_and(|value| matches!(value, Object::String(_, _)));
        if !has_charset {
            self.invalid_type1_subset_charsets.push(font_failure(
                object_id,
                description,
                "is a Type1 subset without a descriptor /CharSet string",
            ));
        }
        Ok(())
    }

    fn inspect_composite_font(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
        rendering_mode: i64,
    ) -> Result<(), PdfError> {
        let descendant = first_descendant_dictionary(self.document, font, self.limits)?;
        if let Some(descendant) = descendant {
            self.inspect_cid_subset_font(descendant, object_id, description)?;
        }
        if let Some(descendant) = descendant
            && descendant
                .get(b"Subtype")
                .ok()
                .and_then(|value| value.as_name().ok())
                == Some(b"CIDFontType2".as_slice())
            && rendering_mode != 3
        {
            let valid_map = match descendant.get(b"CIDToGIDMap") {
                Ok(value) => valid_cid_to_gid_map(self.document, value, self.limits)?,
                Err(_) => false,
            };
            if !valid_map {
                self.invalid_cid_to_gid_maps.push(font_failure(
                    object_id,
                    description,
                    "has a used Type 2 CIDFont descendant without a valid /CIDToGIDMap",
                ));
            }
        }

        let Ok(encoding) = font.get(b"Encoding") else {
            return Ok(());
        };
        if encoding
            .as_name()
            .ok()
            .is_some_and(|name| matches!(name, b"Identity-H" | b"Identity-V"))
        {
            return Ok(());
        }
        let resolved = resolve_optional(self.document, encoding, self.limits.max_reference_depth)?;
        let Some(cmap) = resolved.and_then(|object| object.as_stream().ok()) else {
            self.unembedded_cmaps.push(font_failure(
                object_id,
                description,
                "uses a non-Identity CMap that is not embedded",
            ));
            return Ok(());
        };

        let cmap_name = cmap
            .dict
            .get(b"CMapName")
            .ok()
            .and_then(|value| value.as_name().ok());
        if cmap_name.is_some_and(|name| matches!(name, b"Identity-H" | b"Identity-V")) {
            return Ok(());
        }

        if let Some(descendant) = descendant
            && cid_system_info(self.document, descendant, self.limits)?
                != cid_system_info(self.document, &cmap.dict, self.limits)?
        {
            self.incompatible_type0_system_info.push(font_failure(
                object_id,
                description,
                "has incompatible CIDSystemInfo Registry or Ordering values in its CIDFont and CMap",
            ));
        }

        let dictionary_wmode = cmap
            .dict
            .get(b"WMode")
            .ok()
            .and_then(as_integer)
            .unwrap_or(0);
        let bytes = decode_font_stream(cmap, self.limits)?;
        if cmap_maximal_cid(&bytes).is_some_and(|cid| cid > 65_535) {
            self.invalid_cmap_cids.push(font_failure(
                object_id,
                description,
                "uses an embedded CMap with a CID greater than 65,535",
            ));
        }
        let content_wmode = cmap_content_wmode(&bytes).unwrap_or(0);
        if dictionary_wmode != content_wmode {
            self.invalid_cmap_wmodes.push(font_failure(
                object_id,
                description,
                &format!(
                    "has embedded CMap WMode {content_wmode} but dictionary /WMode {dictionary_wmode}"
                ),
            ));
        }
        Ok(())
    }

    fn inspect_cid_subset_font(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
    ) -> Result<(), PdfError> {
        let is_subset = font
            .get(b"BaseFont")
            .ok()
            .and_then(|value| value.as_name().ok())
            .is_some_and(is_subset_font_name);
        if !is_subset {
            return Ok(());
        }
        let has_cid_set = font_descriptor_dictionary(self.document, font, self.limits)?
            .and_then(|descriptor| descriptor.get(b"CIDSet").ok())
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .is_some_and(|value| value.as_stream().is_ok());
        if !has_cid_set {
            self.invalid_cid_subset_cidsets.push(font_failure(
                object_id,
                description,
                "has a CIDFont subset without a descriptor /CIDSet stream",
            ));
        }
        Ok(())
    }

    fn inspect_font_dictionary(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
        subtype: Option<&str>,
    ) -> Result<(), PdfError> {
        if font
            .get(b"Type")
            .ok()
            .and_then(|value| value.as_name().ok())
            != Some(b"Font".as_slice())
        {
            self.invalid_types.push(font_failure(
                object_id,
                description,
                "has a missing or invalid /Type instead of /Font",
            ));
        }

        let base_font = font
            .get(b"BaseFont")
            .ok()
            .and_then(|value| value.as_name().ok());
        if subtype != Some("Type3") && base_font.is_none() {
            self.invalid_base_fonts.push(font_failure(
                object_id,
                description,
                "has a missing or invalid /BaseFont",
            ));
        }

        if matches!(subtype, Some("Type1" | "MMType1" | "TrueType" | "Type3"))
            && !base_font.is_some_and(is_standard_14_font)
        {
            let first_char = font.get(b"FirstChar").ok().and_then(as_integer);
            let last_char = font.get(b"LastChar").ok().and_then(as_integer);
            if first_char.is_none() {
                self.invalid_first_chars.push(font_failure(
                    object_id,
                    description,
                    "has a missing or invalid /FirstChar",
                ));
            }
            if last_char.is_none() {
                self.invalid_last_chars.push(font_failure(
                    object_id,
                    description,
                    "has a missing or invalid /LastChar",
                ));
            }
            let widths_size = font
                .get(b"Widths")
                .ok()
                .and_then(|value| value.as_array().ok())
                .and_then(|widths| i64::try_from(widths.len()).ok());
            let expected_size = first_char
                .and_then(|first| last_char.and_then(|last| last.checked_sub(first)))
                .and_then(|difference| difference.checked_add(1));
            if widths_size.is_none() || widths_size != expected_size {
                self.invalid_widths.push(font_failure(
                    object_id,
                    description,
                    "has a missing /Widths array or a size different from /LastChar - /FirstChar + 1",
                ));
            }
        }

        if let Some(invalid_subtype) =
            invalid_embedded_font_subtype(self.document, font, self.limits)?
        {
            self.invalid_font_file_subtypes.push(font_failure(
                object_id,
                description,
                &format!("uses unsupported embedded font subtype /{invalid_subtype}"),
            ));
        }
        Ok(())
    }
}

fn first_descendant_dictionary<'a>(
    document: &'a Document,
    font: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a Dictionary>, PdfError> {
    let Ok(descendants) = font.get(b"DescendantFonts") else {
        return Ok(None);
    };
    let Some(descendants) = resolve_optional(document, descendants, limits.max_reference_depth)?
        .and_then(|object| object.as_array().ok())
    else {
        return Ok(None);
    };
    let Some(descendant) = descendants.first() else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, descendant, limits.max_reference_depth)?
            .and_then(|object| object.as_dict().ok()),
    )
}

fn valid_cid_to_gid_map(
    document: &Document,
    value: &Object,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    if value.as_name().ok().is_some_and(|name| name == b"Identity") {
        return Ok(true);
    }
    Ok(
        resolve_optional(document, value, limits.max_reference_depth)?
            .is_some_and(|object| object.as_stream().is_ok()),
    )
}

fn cid_system_info(
    document: &Document,
    dictionary: &Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<CidSystemInfo>, PdfError> {
    let Ok(info) = dictionary.get(b"CIDSystemInfo") else {
        return Ok(None);
    };
    let Some(info) = resolve_optional(document, info, limits.max_reference_depth)?
        .and_then(|object| object.as_dict().ok())
    else {
        return Ok(None);
    };
    let registry = match info.get(b"Registry") {
        Ok(Object::String(value, _)) => value.clone(),
        _ => return Ok(None),
    };
    let ordering = match info.get(b"Ordering") {
        Ok(Object::String(value, _)) => value.clone(),
        _ => return Ok(None),
    };
    Ok(Some((registry, ordering)))
}

fn decode_font_stream(stream: &Stream, limits: &SafetyLimits) -> Result<Vec<u8>, PdfError> {
    match stream.decompressed_content_with_limit(limits.max_decoded_stream_size) {
        Ok(bytes) => Ok(bytes),
        Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })) => {
            Err(PdfError::FontDecodeLimit(limits.max_decoded_stream_size))
        }
        Err(_) => Ok(stream.content.clone()),
    }
}

fn cmap_content_wmode(bytes: &[u8]) -> Option<i64> {
    let mut tokens = bytes
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|token| !token.is_empty());
    while let Some(token) = tokens.next() {
        if token == b"/WMode" {
            return std::str::from_utf8(tokens.next()?).ok()?.parse().ok();
        }
    }
    None
}

fn cmap_maximal_cid(bytes: &[u8]) -> Option<u32> {
    let tokens = bytes
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mut maximum = None;
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        let Some(count) = tokens[cursor]
            .iter()
            .all(u8::is_ascii_digit)
            .then(|| {
                std::str::from_utf8(tokens[cursor])
                    .ok()?
                    .parse::<usize>()
                    .ok()
            })
            .flatten()
        else {
            cursor += 1;
            continue;
        };
        match tokens.get(cursor + 1).copied() {
            Some(b"begincidchar") => {
                for entry in 0..count {
                    if let Some(cid) = tokens
                        .get(cursor + 3 + entry * 2)
                        .and_then(|token| parse_cmap_integer(token))
                    {
                        maximum = Some(maximum.unwrap_or(cid).max(cid));
                    }
                }
                cursor += 2 + count * 2;
            }
            Some(b"begincidrange") => {
                for entry in 0..count {
                    let base = cursor + 2 + entry * 3;
                    let Some(start) = tokens.get(base).and_then(|token| parse_cmap_hex(token))
                    else {
                        continue;
                    };
                    let Some(end) = tokens.get(base + 1).and_then(|token| parse_cmap_hex(token))
                    else {
                        continue;
                    };
                    let Some(cid) = tokens
                        .get(base + 2)
                        .and_then(|token| parse_cmap_integer(token))
                    else {
                        continue;
                    };
                    maximum = Some(
                        maximum
                            .unwrap_or(cid)
                            .max(cid.saturating_add(end.saturating_sub(start))),
                    );
                }
                cursor += 2 + count * 3;
            }
            _ => cursor += 1,
        }
    }
    maximum
}

fn parse_cmap_integer(token: &[u8]) -> Option<u32> {
    std::str::from_utf8(token).ok()?.parse().ok()
}

fn parse_cmap_hex(token: &[u8]) -> Option<u32> {
    let token = token.strip_prefix(b"<")?.strip_suffix(b">")?;
    std::str::from_utf8(token)
        .ok()
        .and_then(|token| u32::from_str_radix(token, 16).ok())
}

fn is_subset_font_name(name: &[u8]) -> bool {
    name.len() >= 7 && name[..6].iter().all(u8::is_ascii_uppercase) && name[6] == b'+'
}

fn font_descriptor_dictionary<'a>(
    document: &'a Document,
    font: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a Dictionary>, PdfError> {
    let Ok(descriptor) = font.get(b"FontDescriptor") else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, descriptor, limits.max_reference_depth)?
            .and_then(|object| object.as_dict().ok()),
    )
}

fn truetype_encoding(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<(Option<Vec<u8>>, bool), PdfError> {
    let Ok(encoding) = font.get(b"Encoding") else {
        return Ok((None, false));
    };
    let Some(encoding) = resolve_optional(document, encoding, limits.max_reference_depth)? else {
        return Ok((None, false));
    };
    if let Ok(name) = encoding.as_name() {
        return Ok((Some(name.to_vec()), false));
    }
    let Ok(dictionary) = encoding.as_dict() else {
        return Ok((None, false));
    };
    let base = dictionary
        .get(b"BaseEncoding")
        .ok()
        .and_then(|value| value.as_name().ok())
        .map(ToOwned::to_owned);
    Ok((base, dictionary.has(b"Differences")))
}

fn truetype_cmap_count(
    document: &Document,
    descriptor: Option<&Dictionary>,
    limits: &SafetyLimits,
) -> Result<Option<usize>, PdfError> {
    let Some(descriptor) = descriptor else {
        return Ok(None);
    };
    let Ok(file) = descriptor.get(b"FontFile2") else {
        return Ok(None);
    };
    let Some(stream) = resolve_optional(document, file, limits.max_reference_depth)?
        .and_then(|object| object.as_stream().ok())
    else {
        return Ok(None);
    };
    let bytes = decode_font_stream(stream, limits)?;
    Ok(ttf_parser::Face::parse(&bytes, 0)
        .ok()
        .and_then(|face| face.tables().cmap)
        .map(|cmap| usize::from(cmap.subtables.len())))
}

fn font_is_embedded(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Ok(descriptor) = font.get(b"FontDescriptor") else {
        return Ok(false);
    };
    let Some(descriptor) = resolve_optional(document, descriptor, limits.max_reference_depth)?
        .and_then(|object| object.as_dict().ok())
    else {
        return Ok(false);
    };
    // `containsFontFile` is a veraPDF model property, not mere key presence:
    // the pinned malformed-program case proves that the stream must be
    // recognized as an embedded font program.
    for key in [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"] {
        if let Ok(file) = descriptor.get(key)
            && let Some(stream) = resolve_optional(document, file, limits.max_reference_depth)?
                .and_then(|object| object.as_stream().ok())
            && valid_font_program(key, stream, limits)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn invalid_embedded_font_subtype(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<String>, PdfError> {
    let Ok(descriptor) = font.get(b"FontDescriptor") else {
        return Ok(None);
    };
    let Some(descriptor) = resolve_optional(document, descriptor, limits.max_reference_depth)?
        .and_then(|object| object.as_dict().ok())
    else {
        return Ok(None);
    };
    for key in [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"] {
        let Ok(file) = descriptor.get(key) else {
            continue;
        };
        let Some(stream) = resolve_optional(document, file, limits.max_reference_depth)?
            .and_then(|object| object.as_stream().ok())
        else {
            continue;
        };
        let Some(subtype) = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
        else {
            continue;
        };
        if !matches!(subtype, b"Type1C" | b"CIDFontType0C") {
            return Ok(Some(String::from_utf8_lossy(subtype).into_owned()));
        }
    }
    Ok(None)
}

fn valid_font_program(
    key: &[u8],
    stream: &Stream,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let bytes = match stream.decompressed_content_with_limit(limits.max_decoded_stream_size) {
        Ok(bytes) => bytes,
        Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })) => {
            return Err(PdfError::FontDecodeLimit(limits.max_decoded_stream_size));
        }
        Err(_) => return Ok(false),
    };
    Ok(match key {
        b"FontFile" => bytes.starts_with(b"%!PS-AdobeFont") || bytes.starts_with(&[0x80, 0x01]),
        b"FontFile2" => valid_sfnt(&bytes),
        b"FontFile3" => {
            matches!(
                stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|value| value.as_name().ok()),
                Some(b"Type1C" | b"CIDFontType0C")
            ) && bytes.len() >= 4
                && matches!(bytes[0], 1 | 2)
                && usize::from(bytes[2]) <= bytes.len()
        }
        _ => false,
    })
}

fn valid_sfnt(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || !matches!(&bytes[..4], b"\0\x01\0\0" | b"true" | b"typ1" | b"OTTO") {
        return false;
    }
    let table_count = usize::from(u16::from_be_bytes([bytes[4], bytes[5]]));
    let Some(directory_end) = table_count
        .checked_mul(16)
        .and_then(|length| 12usize.checked_add(length))
    else {
        return false;
    };
    if directory_end > bytes.len() {
        return false;
    }
    let mut tags = BTreeSet::new();
    for record in bytes[12..directory_end].chunks_exact(16) {
        let offset =
            u32::from_be_bytes(record[8..12].try_into().expect("four-byte table offset")) as usize;
        let length =
            u32::from_be_bytes(record[12..16].try_into().expect("four-byte table length")) as usize;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > bytes.len())
        {
            return false;
        }
        tags.insert(&record[..4]);
    }
    [b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"name"]
        .iter()
        .all(|tag| tags.contains(tag.as_slice()))
}

fn object_key(object: &Object, context: &str, name: Option<&Object>) -> ResourceKey {
    match object {
        Object::Reference(id) => ResourceKey::Indirect(*id),
        _ => ResourceKey::Direct(format!(
            "{context}/{}",
            name.and_then(|value| value.as_name().ok())
                .map(|value| String::from_utf8_lossy(value).into_owned())
                .unwrap_or_else(|| "direct".to_owned())
        )),
    }
}

fn describe_font(object: &Object, context: &str, name: Option<&Object>) -> String {
    match object {
        Object::Reference((number, generation)) => {
            format!("font object {number} {generation}")
        }
        _ => format!(
            "direct font {} in {context}",
            name.and_then(|value| value.as_name().ok())
                .map(|value| format!("/{}", String::from_utf8_lossy(value)))
                .unwrap_or_else(|| "(unnamed)".to_owned())
        ),
    }
}

fn describe_descendant(object: &Object, parent: &str, index: usize) -> String {
    match object {
        Object::Reference((number, generation)) => {
            format!("descendant font object {number} {generation}")
        }
        _ => format!("direct descendant font {index} of {parent}"),
    }
}

fn shows_text(operands: &[Object]) -> bool {
    operands.iter().any(|operand| match operand {
        Object::String(bytes, _) => !bytes.is_empty(),
        Object::Array(items) => shows_text(items),
        _ => false,
    })
}

fn as_integer(object: &Object) -> Option<i64> {
    object.as_i64().ok()
}

fn is_standard_14_font(name: &[u8]) -> bool {
    matches!(
        name,
        b"Courier"
            | b"Courier-Bold"
            | b"Courier-Oblique"
            | b"Courier-BoldOblique"
            | b"Helvetica"
            | b"Helvetica-Bold"
            | b"Helvetica-Oblique"
            | b"Helvetica-BoldOblique"
            | b"Times-Roman"
            | b"Times-Bold"
            | b"Times-Italic"
            | b"Times-BoldItalic"
            | b"Symbol"
            | b"ZapfDingbats"
    )
}

fn font_failure(object_id: Option<PdfObjectId>, description: &str, detail: &str) -> RuleFailure {
    RuleFailure {
        object_id,
        description: format!("{description} {detail}"),
    }
}

#[cfg(test)]
mod tests {
    use lopdf::{Dictionary, Document, Stream};

    use super::{cmap_maximal_cid, inspect_all_embedded_cmap_cids};
    use crate::SafetyLimits;

    #[test]
    fn finds_maximum_cid_in_char_and_range_mappings() {
        assert_eq!(
            cmap_maximal_cid(b"2 begincidchar <00> 10 <01> 65536 endcidchar"),
            Some(65_536)
        );
        assert_eq!(
            cmap_maximal_cid(b"1 begincidrange <00> <FF> 65500 endcidrange"),
            Some(65_755)
        );
        assert_eq!(
            cmap_maximal_cid(b"1 begincidrange <00> <FF> 0 endcidrange"),
            Some(255)
        );
    }

    #[test]
    fn finds_oversized_cids_in_unused_embedded_cmaps() {
        let mut document = Document::with_version("1.4");
        let mut dictionary = Dictionary::new();
        dictionary.set("CMapName", "Unused-CMap");
        document.add_object(Stream::new(
            dictionary,
            b"1 begincidchar <00> 65536 endcidchar".to_vec(),
        ));
        let failures = inspect_all_embedded_cmap_cids(&document, &SafetyLimits::default())
            .expect("inspect CMaps");
        assert_eq!(failures.len(), 1);
    }
}
