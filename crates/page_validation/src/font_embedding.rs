use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::content_support::ContentExecutionSummary;
use crate::error::PdfError;
use crate::font_encodings::{self, PredefinedEncoding};
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{ResourceKey, contains_key, resolve_optional};
use crate::predefined_cmaps;
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
    pub(crate) invalid_font_file_subtypes_pdfa2: Vec<RuleFailure>,
    pub(crate) incompatible_type0_system_info: Vec<RuleFailure>,
    pub(crate) incompatible_type0_system_info_pdfa2: Vec<RuleFailure>,
    pub(crate) invalid_cid_to_gid_maps: Vec<RuleFailure>,
    pub(crate) unembedded_cmaps: Vec<RuleFailure>,
    pub(crate) unembedded_predefined_cmaps: Vec<RuleFailure>,
    pub(crate) invalid_cmap_wmodes: Vec<RuleFailure>,
    pub(crate) invalid_cmap_cids: Vec<RuleFailure>,
    pub(crate) invalid_cmap_references: Vec<RuleFailure>,
    pub(crate) oversized_cmap_cids: Vec<RuleFailure>,
    pub(crate) invalid_type1_subset_charsets: Vec<RuleFailure>,
    pub(crate) invalid_cid_subset_cidsets: Vec<RuleFailure>,
    pub(crate) invalid_nonsymbolic_truetype_encodings: Vec<RuleFailure>,
    pub(crate) invalid_nonsymbolic_truetype_cmaps: Vec<RuleFailure>,
    pub(crate) invalid_symbolic_truetype_encodings: Vec<RuleFailure>,
    pub(crate) invalid_symbolic_truetype_cmaps: Vec<RuleFailure>,
    pub(crate) invalid_unicode_mappings: Vec<RuleFailure>,
    pub(crate) invalid_unicode_values: Vec<RuleFailure>,
    pub(crate) unicode_pua_without_actual_text: Vec<RuleFailure>,
    pub(crate) notdef_glyphs: Vec<RuleFailure>,
    pub(crate) missing_truetype_glyphs: Vec<RuleFailure>,
    pub(crate) missing_type1_glyphs: Vec<RuleFailure>,
    pub(crate) inconsistent_truetype_widths: Vec<RuleFailure>,
    pub(crate) excessive_graphics_state_nesting: Vec<RuleFailure>,
}

#[derive(Debug, PartialEq, Eq)]
struct CidSystemInfo {
    registry: Vec<u8>,
    ordering: Vec<u8>,
    supplement: Option<i64>,
}

impl CidSystemInfo {
    fn matches_pdfa1(&self, other: &Self) -> bool {
        self.registry == other.registry && self.ordering == other.ordering
    }

    fn matches_pdfa2_or_3(&self, cmap: &Self) -> bool {
        self.matches_pdfa1(cmap)
            && self.supplement.zip(cmap.supplement).is_some_and(
                |(cid_font_supplement, cmap_supplement)| cid_font_supplement <= cmap_supplement,
            )
    }
}

#[derive(Clone)]
struct SelectedFont {
    key: ResourceKey,
    object: Object,
    description: String,
}

#[derive(Clone)]
struct FontUse {
    object: Object,
    object_id: Option<PdfObjectId>,
    description: String,
    subtype: Option<String>,
    embedded: bool,
    visible: bool,
    shown_bytes: Vec<u8>,
    shown_text_actual_text: Vec<(Vec<u8>, bool)>,
}

struct Scanner<'a> {
    document: &'a Document,
    limits: &'a SafetyLimits,
    /// Embedded CMaps parsed once per indirect object and reused across the
    /// several checks that otherwise each independently decode and
    /// re-tokenize the same CMap stream for the same font.
    cmap_decoders: HashMap<ObjectId, Rc<CmapDecoder>>,
    uses: BTreeMap<ResourceKey, FontUse>,
    active_descendant_fonts: BTreeSet<ResourceKey>,
    invalid_types: Vec<RuleFailure>,
    invalid_subtypes: Vec<RuleFailure>,
    invalid_base_fonts: Vec<RuleFailure>,
    invalid_first_chars: Vec<RuleFailure>,
    invalid_last_chars: Vec<RuleFailure>,
    invalid_widths: Vec<RuleFailure>,
    invalid_font_file_subtypes: Vec<RuleFailure>,
    invalid_font_file_subtypes_pdfa2: Vec<RuleFailure>,
    incompatible_type0_system_info: Vec<RuleFailure>,
    incompatible_type0_system_info_pdfa2: Vec<RuleFailure>,
    invalid_cid_to_gid_maps: Vec<RuleFailure>,
    unembedded_cmaps: Vec<RuleFailure>,
    unembedded_predefined_cmaps: Vec<RuleFailure>,
    invalid_cmap_wmodes: Vec<RuleFailure>,
    invalid_cmap_cids: Vec<RuleFailure>,
    invalid_cmap_references: Vec<RuleFailure>,
    oversized_cmap_cids: Vec<RuleFailure>,
    invalid_type1_subset_charsets: Vec<RuleFailure>,
    invalid_cid_subset_cidsets: Vec<RuleFailure>,
    invalid_nonsymbolic_truetype_encodings: Vec<RuleFailure>,
    invalid_nonsymbolic_truetype_cmaps: Vec<RuleFailure>,
    invalid_symbolic_truetype_encodings: Vec<RuleFailure>,
    invalid_symbolic_truetype_cmaps: Vec<RuleFailure>,
    invalid_unicode_mappings: Vec<RuleFailure>,
    invalid_unicode_values: Vec<RuleFailure>,
    unicode_pua_without_actual_text: Vec<RuleFailure>,
    notdef_glyphs: Vec<RuleFailure>,
    missing_truetype_glyphs: Vec<RuleFailure>,
    missing_type1_glyphs: Vec<RuleFailure>,
    inconsistent_truetype_widths: Vec<RuleFailure>,
    excessive_graphics_state_nesting: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    execution: &ContentExecutionSummary,
    limits: &SafetyLimits,
) -> Result<FontEmbeddingSummary, PdfError> {
    let mut scanner = Scanner {
        document,
        limits,
        cmap_decoders: HashMap::new(),
        uses: BTreeMap::new(),
        active_descendant_fonts: BTreeSet::new(),
        invalid_types: Vec::new(),
        invalid_subtypes: Vec::new(),
        invalid_base_fonts: Vec::new(),
        invalid_first_chars: Vec::new(),
        invalid_last_chars: Vec::new(),
        invalid_widths: Vec::new(),
        invalid_font_file_subtypes: Vec::new(),
        invalid_font_file_subtypes_pdfa2: Vec::new(),
        incompatible_type0_system_info: Vec::new(),
        incompatible_type0_system_info_pdfa2: Vec::new(),
        invalid_cid_to_gid_maps: Vec::new(),
        unembedded_cmaps: Vec::new(),
        unembedded_predefined_cmaps: Vec::new(),
        invalid_cmap_wmodes: Vec::new(),
        invalid_cmap_cids: Vec::new(),
        invalid_cmap_references: Vec::new(),
        oversized_cmap_cids: Vec::new(),
        invalid_type1_subset_charsets: Vec::new(),
        invalid_cid_subset_cidsets: Vec::new(),
        invalid_nonsymbolic_truetype_encodings: Vec::new(),
        invalid_nonsymbolic_truetype_cmaps: Vec::new(),
        invalid_symbolic_truetype_encodings: Vec::new(),
        invalid_symbolic_truetype_cmaps: Vec::new(),
        invalid_unicode_mappings: Vec::new(),
        invalid_unicode_values: Vec::new(),
        unicode_pua_without_actual_text: Vec::new(),
        notdef_glyphs: Vec::new(),
        missing_truetype_glyphs: Vec::new(),
        missing_type1_glyphs: Vec::new(),
        inconsistent_truetype_widths: Vec::new(),
        excessive_graphics_state_nesting: execution.excessive_graphics_state_nesting.clone(),
    };
    for usage in &execution.fonts {
        scanner.record_font(
            &SelectedFont {
                key: usage.key.clone(),
                object: usage.object.clone(),
                description: usage.description.clone(),
            },
            usage.rendering_mode,
            &usage.shown_bytes,
            usage.actual_text_present,
        )?;
    }
    scanner.oversized_cmap_cids = inspect_all_embedded_cmap_cids(document, limits)?;
    scanner.inspect_rendered_notdef_glyphs()?;
    scanner.invalid_cmap_cids.sort_by(|left, right| {
        left.object_id
            .cmp(&right.object_id)
            .then_with(|| left.description.cmp(&right.description))
    });
    scanner.invalid_cmap_cids.dedup_by(|left, right| {
        left.object_id == right.object_id && left.description == right.description
    });
    scanner.inspect_rendered_truetype_glyphs()?;
    scanner.inspect_rendered_type1_glyphs()?;
    scanner.inspect_rendered_type3_glyphs()?;
    scanner.notdef_glyphs.sort_by(|left, right| {
        left.object_id
            .cmp(&right.object_id)
            .then_with(|| left.description.cmp(&right.description))
    });
    scanner.notdef_glyphs.dedup_by(|left, right| {
        left.object_id == right.object_id && left.description == right.description
    });
    scanner.inspect_rendered_cff_type1_glyphs()?;
    scanner.inspect_rendered_cidfont_glyphs()?;
    scanner.inspect_rendered_unicode_mappings()?;
    scanner.inspect_unicode_pua_actual_text()?;
    scanner.inspect_rendered_type1_subset_charsets()?;
    scanner.inspect_rendered_cid_subset_sets()?;

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
        invalid_font_file_subtypes_pdfa2: scanner.invalid_font_file_subtypes_pdfa2,
        incompatible_type0_system_info: scanner.incompatible_type0_system_info,
        incompatible_type0_system_info_pdfa2: scanner.incompatible_type0_system_info_pdfa2,
        invalid_cid_to_gid_maps: scanner.invalid_cid_to_gid_maps,
        unembedded_cmaps: scanner.unembedded_cmaps,
        unembedded_predefined_cmaps: scanner.unembedded_predefined_cmaps,
        invalid_cmap_wmodes: scanner.invalid_cmap_wmodes,
        invalid_cmap_cids: scanner.invalid_cmap_cids,
        invalid_cmap_references: scanner.invalid_cmap_references,
        oversized_cmap_cids: scanner.oversized_cmap_cids,
        invalid_type1_subset_charsets: scanner.invalid_type1_subset_charsets,
        invalid_cid_subset_cidsets: scanner.invalid_cid_subset_cidsets,
        invalid_nonsymbolic_truetype_encodings: scanner.invalid_nonsymbolic_truetype_encodings,
        invalid_nonsymbolic_truetype_cmaps: scanner.invalid_nonsymbolic_truetype_cmaps,
        invalid_symbolic_truetype_encodings: scanner.invalid_symbolic_truetype_encodings,
        invalid_symbolic_truetype_cmaps: scanner.invalid_symbolic_truetype_cmaps,
        invalid_unicode_mappings: scanner.invalid_unicode_mappings,
        invalid_unicode_values: scanner.invalid_unicode_values,
        unicode_pua_without_actual_text: scanner.unicode_pua_without_actual_text,
        notdef_glyphs: scanner.notdef_glyphs,
        missing_truetype_glyphs: scanner.missing_truetype_glyphs,
        missing_type1_glyphs: scanner.missing_type1_glyphs,
        inconsistent_truetype_widths: scanner.inconsistent_truetype_widths,
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
    fn record_font(
        &mut self,
        selected: &SelectedFont,
        rendering_mode: i64,
        shown_bytes: &[u8],
        actual_text_present: bool,
    ) -> Result<(), PdfError> {
        if let Some(font_use) = self.uses.get_mut(&selected.key) {
            font_use
                .shown_text_actual_text
                .push((shown_bytes.to_vec(), actual_text_present));
            if rendering_mode != 3 {
                font_use.shown_bytes.extend_from_slice(shown_bytes);
            }
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
        let subtype = resolved_name(self.document, font, b"Subtype", self.limits)?
            .map(|value| String::from_utf8_lossy(value).into_owned());
        // veraPDF 1.30.2 does not create a PDFont model object for a missing
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
            self.inspect_composite_font(font, object_id, &selected.description)?;
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
            object: selected.object.clone(),
            object_id,
            description: selected.description.clone(),
            subtype: subtype.clone(),
            embedded,
            // veraPDF 1.30.2 associates the first observed rendering mode
            // with a font model object and does not revise it on later uses.
            visible: rendering_mode != 3,
            shown_bytes: if rendering_mode == 3 {
                Vec::new()
            } else {
                shown_bytes.to_vec()
            },
            shown_text_actual_text: vec![(shown_bytes.to_vec(), actual_text_present)],
        });

        if subtype.as_deref() == Some("Type0")
            && self.active_descendant_fonts.insert(selected.key.clone())
        {
            // PDF32000 9.7.3 requires exactly one entry, and veraPDF 1.30.2
            // confirms this in its object model: it creates a PDCIDFont (and
            // evaluates every per-font predicate, including embedding) only
            // for DescendantFonts[0]. A second or later entry is invisible
            // to it entirely, so recording it here too would make a local
            // check fail (e.g. embedding) for an object veraPDF never
            // examines -- a confirmed false positive, not extra coverage.
            if let Ok(descendants) = font.get(b"DescendantFonts")
                && let Some(descendants) =
                    resolve_optional(self.document, descendants, self.limits.max_reference_depth)?
                        .and_then(|object| object.as_array().ok())
                && let Some(descendant) = descendants.first()
            {
                let context = format!("{}/descendant 0", selected.description);
                let descendant = SelectedFont {
                    key: object_key(descendant, &context, None),
                    object: descendant.clone(),
                    description: describe_descendant(descendant, &selected.description),
                };
                self.record_font(&descendant, rendering_mode, &[], false)?;
            }
            self.active_descendant_fonts.remove(&selected.key);
        }
        Ok(())
    }

    /// Resolves the CIDs a Type0 font's `encoding` maps `shown_bytes`
    /// through, reusing a previously decoded/parsed embedded CMap for the
    /// same indirect object instead of re-decompressing and re-tokenizing it.
    /// The two glyph-presence and width checks that need this both look it
    /// up for the same font, so a cache hit is the common case.
    fn cached_cids_for_rendered_bytes(
        &mut self,
        encoding: &Object,
        shown_bytes: &[u8],
    ) -> Result<Option<Vec<u16>>, PdfError> {
        let Some(object_id) = encoding.as_reference().ok() else {
            let decoder = resolve_cmap_decoder(self.document, encoding, self.limits)?;
            return Ok(decoder.decode(shown_bytes));
        };
        let decoder = match self.cmap_decoders.get(&object_id) {
            Some(decoder) => Rc::clone(decoder),
            None => {
                let decoder = Rc::new(resolve_cmap_decoder(self.document, encoding, self.limits)?);
                self.cmap_decoders.insert(object_id, Rc::clone(&decoder));
                decoder
            }
        };
        Ok(decoder.decode(shown_bytes))
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
            .map(|value| resolved_integer(self.document, self.limits, value))
            .transpose()?
            .flatten()
            .is_some_and(|flags| flags & 4 != 0);
        let (encoding, contains_differences, differences_unicode_compliant) =
            truetype_encoding(self.document, font, self.limits)?;

        if symbolic {
            if encoding.is_some() {
                self.invalid_symbolic_truetype_encodings.push(font_failure(
                    object_id,
                    description,
                    "is symbolic but specifies an /Encoding",
                ));
            }
            if let Some((cmap_count, cmap30_present)) =
                truetype_cmap_summary(self.document, descriptor, self.limits)?
                && cmap_count != 1
                && !cmap30_present
            {
                self.invalid_symbolic_truetype_cmaps.push(font_failure(
                    object_id,
                    description,
                    &format!(
                        "is symbolic but its embedded TrueType program contains {cmap_count} cmap subtables and no Microsoft Symbol cmap"
                    ),
                ));
            }
        } else {
            if font_is_embedded(self.document, font, self.limits)?
                && let Some((cmap_count, cmap30_present)) =
                    truetype_cmap_summary(self.document, descriptor, self.limits)?
                && ((cmap30_present && cmap_count <= 1) || (!cmap30_present && cmap_count == 0))
            {
                self.invalid_nonsymbolic_truetype_cmaps.push(font_failure(
                    object_id,
                    description,
                    &format!(
                        "is non-symbolic but its embedded TrueType program has {cmap_count} cmap subtables{}",
                        if cmap30_present {
                            " including a Microsoft Symbol cmap"
                        } else {
                            " and no Microsoft Symbol cmap"
                        }
                    ),
                ));
            }
            if !matches!(
                encoding.as_deref(),
                Some(b"MacRomanEncoding" | b"WinAnsiEncoding")
            ) || (contains_differences && !differences_unicode_compliant)
            {
                self.invalid_nonsymbolic_truetype_encodings
                    .push(font_failure(
                        object_id,
                        description,
                        "is non-symbolic but lacks an unmodified MacRomanEncoding or WinAnsiEncoding",
                    ));
            }
        }
        Ok(())
    }

    fn inspect_rendered_unicode_mappings(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible || usage.shown_bytes.is_empty() {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else { continue };
            if unicode_mapping_exception(
                self.document,
                font,
                usage.subtype.as_deref(),
                &usage.shown_bytes,
                self.limits,
            )? {
                continue;
            }
            let Some(value) = font.get(b"ToUnicode").ok() else {
                self.invalid_unicode_mappings.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "does not define a ToUnicode CMap for rendered text",
                ));
                continue;
            };
            let Some(value) =
                resolve_optional(self.document, value, self.limits.max_reference_depth)?
            else {
                self.invalid_unicode_mappings.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "has an unresolved ToUnicode entry",
                ));
                continue;
            };
            let Some(stream) = value.as_stream().ok() else {
                self.invalid_unicode_mappings.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "has a ToUnicode entry that is not a CMap stream",
                ));
                continue;
            };
            let bytes = decode_font_stream(stream, self.limits)?;
            let Some(map) = UnicodeCmap::parse(&bytes) else {
                self.invalid_unicode_mappings.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "has a malformed ToUnicode CMap",
                ));
                continue;
            };
            if map.has_reserved_values {
                self.invalid_unicode_values.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "has a ToUnicode CMap value of U+0000, U+FEFF, or U+FFFE",
                ));
            }
            let rendered_codes = if usage.subtype.as_deref() == Some("Type0") {
                let Some(encoding) = font.get(b"Encoding").ok() else {
                    continue;
                };
                let Some(decoder) = resolve_cmap_decoder(self.document, encoding, self.limits)?
                    .codes(&usage.shown_bytes)
                else {
                    self.invalid_unicode_mappings.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        "has rendered codes that cannot be decoded",
                    ));
                    continue;
                };
                decoder
            } else {
                usage.shown_bytes.iter().map(|byte| vec![*byte]).collect()
            };
            if rendered_codes.iter().any(|code| !map.maps_usable(code)) {
                self.invalid_unicode_mappings.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "has rendered character codes missing from or invalid in its ToUnicode CMap",
                ));
            }
        }
        Ok(())
    }

    fn inspect_unicode_pua_actual_text(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if usage.shown_text_actual_text.is_empty() {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else { continue };
            let Some(value) = font.get(b"ToUnicode").ok() else {
                continue;
            };
            let Some(stream) =
                resolve_optional(self.document, value, self.limits.max_reference_depth)?
                    .and_then(|value| value.as_stream().ok())
            else {
                continue;
            };
            let Some(map) = UnicodeCmap::parse(&decode_font_stream(stream, self.limits)?) else {
                continue;
            };
            for (shown_bytes, actual_text_present) in usage.shown_text_actual_text {
                if actual_text_present {
                    continue;
                }
                let rendered_codes = if usage.subtype.as_deref() == Some("Type0") {
                    let Some(encoding) = font.get(b"Encoding").ok() else {
                        continue;
                    };
                    let Some(codes) = resolve_cmap_decoder(self.document, encoding, self.limits)?
                        .codes(&shown_bytes)
                    else {
                        continue;
                    };
                    codes
                } else {
                    shown_bytes.iter().map(|byte| vec![*byte]).collect()
                };
                if rendered_codes.iter().any(|code| map.maps_pua(code)) {
                    self.unicode_pua_without_actual_text.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        "maps rendered text to the Unicode Private Use Area without enclosing /ActualText",
                    ));
                    break;
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_truetype_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || !usage.embedded
                || usage.subtype.as_deref() != Some("TrueType")
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Some(descriptor) = font_descriptor_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let symbolic = descriptor
                .get(b"Flags")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten()
                .is_some_and(|flags| flags & 4 != 0);
            let (encoding, contains_differences, differences_unicode_compliant) =
                truetype_encoding(self.document, font, self.limits)?;
            if (!symbolic
                && (!matches!(
                    encoding.as_deref(),
                    Some(b"MacRomanEncoding" | b"WinAnsiEncoding")
                ) || (contains_differences && !differences_unicode_compliant)))
                || (symbolic && encoding.is_some())
            {
                continue;
            }
            let Ok(file) = descriptor.get(b"FontFile2") else {
                continue;
            };
            let Some(stream) =
                resolve_optional(self.document, file, self.limits.max_reference_depth)?
                    .and_then(|object| object.as_stream().ok())
            else {
                continue;
            };
            let bytes = decode_font_stream(stream, self.limits)?;
            let Some(face) = RawTrueType::parse(&bytes) else {
                continue;
            };
            let first_char = font
                .get(b"FirstChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
            let widths = resolved_array(self.document, font, b"Widths", self.limits)?;
            let mut encoding_dictionary = Dictionary::new();
            let pdf_encoding = if let Some(encoding) = encoding.as_deref() {
                encoding_dictionary.set("Type", "Font");
                encoding_dictionary.set("Encoding", Object::Name(encoding.to_vec()));
                encoding_dictionary.get_font_encoding(self.document).ok()
            } else {
                None
            };
            for byte in usage.shown_bytes.into_iter().collect::<BTreeSet<_>>() {
                let glyph = if symbolic {
                    face.glyph_index_for_symbolic_byte(byte)
                } else {
                    pdf_encoding
                        .as_ref()
                        .and_then(|encoding| single_encoded_character(encoding, byte))
                        .and_then(|character| face.glyph_index(character))
                };
                if glyph.is_some_and(|glyph| glyph.0 == 0) {
                    self.notdef_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        "references the .notdef glyph for a rendered byte",
                    ));
                }
                let Some(glyph) = glyph.filter(|glyph| face.glyph_is_present(*glyph)) else {
                    self.missing_truetype_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!("has no embedded TrueType glyph for rendered byte {byte}"),
                    ));
                    continue;
                };
                let (Some(first_char), Some(widths), Some(advance), Some(units_per_em)) = (
                    first_char,
                    widths,
                    face.glyph_hor_advance(glyph),
                    face.units_per_em,
                ) else {
                    continue;
                };
                let Some(index) = i64::from(byte).checked_sub(first_char) else {
                    continue;
                };
                let Ok(index) = usize::try_from(index) else {
                    continue;
                };
                let Some(dictionary_width) = widths
                    .get(index)
                    .map(|value| resolved_float(self.document, self.limits, value))
                    .transpose()?
                    .flatten()
                else {
                    continue;
                };
                let program_width = f64::from(advance) * 1000.0 / f64::from(units_per_em);
                if (program_width - f64::from(dictionary_width)).abs() > 1.0 {
                    self.inconsistent_truetype_widths.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!(
                            "has rendered byte {byte} width {program_width:.3} in its embedded TrueType program but {dictionary_width:.3} in /Widths"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_notdef_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if usage.shown_bytes.is_empty() {
                continue;
            }
            let Some(font) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            .and_then(|object| object.as_dict().ok()) else {
                continue;
            };
            if usage.subtype.as_deref() == Some("Type0") {
                let Some(encoding) = font.get(b"Encoding").ok() else {
                    continue;
                };
                if resolve_cmap_decoder(self.document, encoding, self.limits)?
                    .decode(&usage.shown_bytes)
                    .is_some_and(|cids| cids.contains(&0))
                {
                    self.notdef_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        "references the .notdef glyph for a rendered CID",
                    ));
                }
            } else if matches!(
                usage.subtype.as_deref(),
                Some("Type1" | "MMType1" | "TrueType" | "Type3")
            ) {
                let encoding = simple_font_encoding(self.document, font, self.limits)?;
                if usage
                    .shown_bytes
                    .iter()
                    .any(|byte| encoding.glyph_name(*byte) == Some(".notdef"))
                {
                    self.notdef_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        "references the .notdef glyph for a rendered byte",
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_cidfont_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || usage.subtype.as_deref() != Some("Type0")
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Ok(encoding) = font.get(b"Encoding") else {
                continue;
            };
            let Some(cids) = self.cached_cids_for_rendered_bytes(encoding, &usage.shown_bytes)?
            else {
                continue;
            };
            let Some(descendant) = first_descendant_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let descendant_subtype =
                resolved_name(self.document, descendant, b"Subtype", self.limits)?;
            if descendant_subtype == Some(b"CIDFontType0".as_slice()) {
                self.inspect_rendered_cff_cidfont_glyphs(&usage, descendant, &cids)?;
                continue;
            }
            if descendant_subtype != Some(b"CIDFontType2".as_slice()) {
                continue;
            }
            let Ok(cid_to_gid) = descendant.get(b"CIDToGIDMap") else {
                continue;
            };
            let Some(descriptor) =
                font_descriptor_dictionary(self.document, descendant, self.limits)?
            else {
                continue;
            };
            let Some(stream) = descriptor
                .get(b"FontFile2")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            else {
                continue;
            };
            let bytes = decode_font_stream(stream, self.limits)?;
            let Some(face) = RawTrueType::parse(&bytes) else {
                continue;
            };
            // Resolved/parsed once per font instead of once per rendered CID.
            let cid_to_gid_map = resolve_cid_to_gid_map(self.document, cid_to_gid, self.limits)?;
            let cid_widths = parse_cid_widths(self.document, descendant, self.limits)?;
            for cid in cids.into_iter().collect::<BTreeSet<_>>() {
                if cid == 0 {
                    self.notdef_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        "references the .notdef glyph for a rendered CID",
                    ));
                    continue;
                }
                let Some(glyph) = cid_to_gid_map.glyph_for(cid) else {
                    continue;
                };
                if !face.glyph_is_present(glyph) {
                    self.missing_truetype_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!("has no embedded CIDFontType2 glyph for rendered CID {cid}"),
                    ));
                    continue;
                }
                let (Some(advance), Some(units_per_em), Some(dictionary_width)) = (
                    face.glyph_hor_advance(glyph),
                    face.units_per_em,
                    cid_widths.width_for(cid),
                ) else {
                    continue;
                };
                let program_width = f64::from(advance) * 1000.0 / f64::from(units_per_em);
                if (program_width - dictionary_width).abs() > 1.0 {
                    self.inconsistent_truetype_widths.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!(
                            "has rendered CID {cid} width {program_width:.3} in its embedded CIDFontType2 program but {dictionary_width:.3} in /DW"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_cff_cidfont_glyphs(
        &mut self,
        usage: &FontUse,
        descendant: &Dictionary,
        cids: &[u16],
    ) -> Result<(), PdfError> {
        let Some(descriptor) = font_descriptor_dictionary(self.document, descendant, self.limits)?
        else {
            return Ok(());
        };
        let Some(stream) = descriptor
            .get(b"FontFile3")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|value| value.as_stream().ok())
        else {
            return Ok(());
        };
        if resolved_name(self.document, &stream.dict, b"Subtype", self.limits)?
            != Some(b"CIDFontType0C".as_slice())
        {
            return Ok(());
        }
        let bytes = decode_font_stream(stream, self.limits)?;
        let Some(cff) = ttf_parser::cff::Table::parse(&bytes) else {
            return Ok(());
        };
        // Built once per font instead of scanning every glyph per rendered
        // CID; `or_insert` keeps the same lowest-glyph-id tie-break as the
        // original `.find()` over an ascending glyph-id range.
        let mut glyph_by_cid: HashMap<u16, ttf_parser::GlyphId> = HashMap::new();
        for glyph in (0..cff.number_of_glyphs()).map(ttf_parser::GlyphId) {
            if let Some(cid) = cff.glyph_cid(glyph) {
                glyph_by_cid.entry(cid).or_insert(glyph);
            }
        }
        let cid_widths = parse_cid_widths(self.document, descendant, self.limits)?;
        for cid in cids.iter().copied().collect::<BTreeSet<_>>() {
            if cid == 0 {
                self.notdef_glyphs.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "references the .notdef glyph for a rendered CID",
                ));
                continue;
            }
            let Some(&glyph) = glyph_by_cid.get(&cid) else {
                self.missing_truetype_glyphs.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    &format!("has no embedded CIDFontType0C glyph for rendered CID {cid}"),
                ));
                continue;
            };
            let Some(program_width) = cff_cid_glyph_width(&bytes, glyph.0) else {
                continue;
            };
            let Some(dictionary_width) = cid_widths.width_for(cid) else {
                continue;
            };
            if (program_width - dictionary_width).abs() > 1.0 {
                self.inconsistent_truetype_widths.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    &format!(
                        "has rendered CID {cid} width {program_width:.3} in its embedded CIDFontType0C program but {dictionary_width:.3} in /W or /DW"
                    ),
                ));
            }
        }
        Ok(())
    }

    fn inspect_rendered_type1_subset_charsets(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || !matches!(usage.subtype.as_deref(), Some("Type1" | "MMType1"))
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            if !resolved_name(self.document, font, b"BaseFont", self.limits)?
                .is_some_and(is_subset_font_name)
            {
                continue;
            }
            let Some(descriptor) = font_descriptor_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let Some(char_set_bytes) =
                resolved_string(self.document, descriptor, b"CharSet", self.limits)?
            else {
                continue;
            };
            let char_set = type1_charset_names(&char_set_bytes);
            let invalid = if let Some(stream) = descriptor
                .get(b"FontFile")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            {
                let program_bytes = decode_font_stream(stream, self.limits)?;
                let program_names = type1_program_char_names(&program_bytes);
                let encoding = simple_font_encoding(self.document, font, self.limits)?;
                let rendered_bytes = usage.shown_bytes.into_iter().collect::<BTreeSet<_>>();
                program_names.is_empty()
                    || rendered_bytes.iter().copied().any(|byte| {
                        let name = encoding.glyph_name(byte);
                        name.is_some_and(|name| {
                            program_names.contains(name) && !char_set.contains(name)
                        })
                    })
            } else if let Some(stream) = descriptor
                .get(b"FontFile3")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            {
                if resolved_name(self.document, &stream.dict, b"Subtype", self.limits)?
                    != Some(b"Type1C".as_slice())
                {
                    true
                } else if let Some(cff) =
                    ttf_parser::cff::Table::parse(&decode_font_stream(stream, self.limits)?)
                {
                    let encoding = simple_font_encoding(self.document, font, self.limits)?;
                    let rendered_bytes = usage.shown_bytes.into_iter().collect::<BTreeSet<_>>();
                    rendered_bytes.iter().copied().any(|byte| {
                        encoding
                            .glyph_name(byte)
                            .and_then(|name| cff.glyph_index_by_name(name))
                            .and_then(|glyph| cff.glyph_name(glyph))
                            .is_some_and(|name| !char_set.contains(name))
                    })
                } else {
                    true
                }
            } else {
                true
            };
            if invalid {
                self.invalid_type1_subset_charsets.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "has a rendered Type1 glyph absent from its descriptor /CharSet",
                ));
            }
        }
        Ok(())
    }

    fn inspect_rendered_type1_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || !usage.embedded
                || !matches!(usage.subtype.as_deref(), Some("Type1" | "MMType1"))
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Some(descriptor) = font_descriptor_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let Some(stream) = descriptor
                .get(b"FontFile")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            else {
                continue;
            };
            let program_bytes = decode_font_stream(stream, self.limits)?;
            let program_names = type1_program_char_names(&program_bytes);
            let program_widths = type1_program_charstring_widths(&program_bytes);
            let program_encoding = type1_program_encoding(&program_bytes);
            if program_names.is_empty() {
                continue;
            }
            let encoding = simple_font_encoding(self.document, font, self.limits)?;
            let first_char = font
                .get(b"FirstChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
            let widths = resolved_array(self.document, font, b"Widths", self.limits)?;
            for byte in usage.shown_bytes.into_iter().collect::<BTreeSet<_>>() {
                let Some(name) = encoding
                    .glyph_name(byte)
                    .or_else(|| program_encoding.glyph_name(byte))
                else {
                    continue;
                };
                if name == ".notdef" {
                    self.notdef_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        "references the .notdef glyph for a rendered byte",
                    ));
                }
                let glyph_present = program_names.contains(name);
                if !glyph_present {
                    self.missing_type1_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!("has no embedded Type1 glyph for rendered byte {byte}"),
                    ));
                }
                let program_width = program_widths.get(name).copied().unwrap_or(0.0);
                let (Some(first_char), Some(widths)) = (first_char, widths) else {
                    continue;
                };
                let Some(index) = i64::from(byte).checked_sub(first_char) else {
                    continue;
                };
                let Ok(index) = usize::try_from(index) else {
                    continue;
                };
                let Some(dictionary_width) = widths
                    .get(index)
                    .map(|value| resolved_float(self.document, self.limits, value))
                    .transpose()?
                    .flatten()
                else {
                    continue;
                };
                if (program_width - f64::from(dictionary_width)).abs() > 1.0 {
                    self.inconsistent_truetype_widths.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!(
                            "has rendered byte {byte} width {program_width:.3} in its embedded Type1 program but {dictionary_width:.3} in /Widths"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_type3_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || usage.subtype.as_deref() != Some("Type3")
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Some(char_procs) = font
                .get(b"CharProcs")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_dict().ok())
            else {
                continue;
            };
            let encoding = simple_font_encoding(self.document, font, self.limits)?;
            let first_char = font
                .get(b"FirstChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
            let widths = resolved_array(self.document, font, b"Widths", self.limits)?;
            for byte in usage.shown_bytes.into_iter().collect::<BTreeSet<_>>() {
                let Some(name) = encoding.glyph_name(byte) else {
                    continue;
                };
                if name == ".notdef" {
                    self.notdef_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        "references the .notdef glyph for a rendered byte",
                    ));
                }
                let charproc = char_procs
                    .get(name.as_bytes())
                    .ok()
                    .map(|value| {
                        resolve_optional(self.document, value, self.limits.max_reference_depth)
                    })
                    .transpose()?
                    .flatten();
                if charproc.is_none() {
                    self.missing_type1_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!("has no Type3 /CharProcs entry for rendered byte {byte}"),
                    ));
                }
                let program_width = match charproc.and_then(|value| value.as_stream().ok()) {
                    Some(stream) => type3_charproc_width(stream, self.limits)?.unwrap_or(0.0),
                    None => 0.0,
                };
                let (Some(first_char), Some(widths)) = (first_char, widths) else {
                    continue;
                };
                let Some(index) = i64::from(byte).checked_sub(first_char) else {
                    continue;
                };
                let Ok(index) = usize::try_from(index) else {
                    continue;
                };
                let Some(dictionary_width) = widths
                    .get(index)
                    .map(|value| resolved_float(self.document, self.limits, value))
                    .transpose()?
                    .flatten()
                else {
                    continue;
                };
                if (program_width - f64::from(dictionary_width)).abs() > 1.0 {
                    self.inconsistent_truetype_widths.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!(
                            "has rendered Type3 byte {byte} width {program_width:.3} in its /CharProc but {dictionary_width:.3} in /Widths"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_cff_type1_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || !usage.embedded
                || !matches!(usage.subtype.as_deref(), Some("Type1" | "MMType1"))
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Some(descriptor) = font_descriptor_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let Some(stream) = descriptor
                .get(b"FontFile3")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            else {
                continue;
            };
            if resolved_name(self.document, &stream.dict, b"Subtype", self.limits)?
                != Some(b"Type1C".as_slice())
            {
                continue;
            }
            let bytes = decode_font_stream(stream, self.limits)?;
            let Some(cff) = ttf_parser::cff::Table::parse(&bytes) else {
                continue;
            };
            let encoding = simple_font_encoding(self.document, font, self.limits)?;
            let first_char = font
                .get(b"FirstChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
            let widths = resolved_array(self.document, font, b"Widths", self.limits)?;
            for byte in usage.shown_bytes.into_iter().collect::<BTreeSet<_>>() {
                let glyph = cff_glyph_for_byte(&cff, &encoding, byte);
                let Some(glyph) = glyph else {
                    self.missing_type1_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!("has no embedded Type1C glyph for rendered byte {byte}"),
                    ));
                    continue;
                };
                let (Some(first_char), Some(widths), Some(width)) =
                    (first_char, widths, cff.glyph_width(glyph))
                else {
                    continue;
                };
                let Some(index) = i64::from(byte).checked_sub(first_char) else {
                    continue;
                };
                let Ok(index) = usize::try_from(index) else {
                    continue;
                };
                let Some(dictionary_width) = widths
                    .get(index)
                    .map(|value| resolved_float(self.document, self.limits, value))
                    .transpose()?
                    .flatten()
                else {
                    continue;
                };
                let program_width = f64::from(width) * f64::from(cff.matrix().sx) * 1000.0;
                if (program_width - f64::from(dictionary_width)).abs() > 1.0 {
                    self.inconsistent_truetype_widths.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!(
                            "has rendered byte {byte} width {program_width:.3} in its embedded Type1C program but {dictionary_width:.3} in /Widths"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_cid_subset_sets(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || usage.subtype.as_deref() != Some("Type0")
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Ok(encoding) = font.get(b"Encoding") else {
                continue;
            };
            let Some(cids) = self.cached_cids_for_rendered_bytes(encoding, &usage.shown_bytes)?
            else {
                continue;
            };
            let Some(descendant) = first_descendant_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let descendant_subtype =
                resolved_name(self.document, descendant, b"Subtype", self.limits)?;
            if !matches!(descendant_subtype, Some(b"CIDFontType2" | b"CIDFontType0"))
                || !resolved_name(self.document, descendant, b"BaseFont", self.limits)?
                    .is_some_and(is_subset_font_name)
            {
                continue;
            }
            let Some(descriptor) =
                font_descriptor_dictionary(self.document, descendant, self.limits)?
            else {
                continue;
            };
            let Some(cid_set) = descriptor
                .get(b"CIDSet")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            else {
                continue;
            };
            let cid_set_bytes = decode_font_stream(cid_set, self.limits)?;
            if !cids
                .into_iter()
                .any(|cid| cid != 0 && !cid_set_contains(&cid_set_bytes, cid))
            {
                continue;
            }
            self.invalid_cid_subset_cidsets.push(font_failure(
                usage.object_id,
                &usage.description,
                "has a rendered CID absent from its descriptor /CIDSet",
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
        let is_subset = resolved_name(self.document, font, b"BaseFont", self.limits)?
            .is_some_and(is_subset_font_name);
        if !is_subset {
            return Ok(());
        }
        let has_charset = font_descriptor_dictionary(self.document, font, self.limits)?
            .map(|descriptor| resolved_string(self.document, descriptor, b"CharSet", self.limits))
            .transpose()?
            .flatten()
            .is_some();
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
    ) -> Result<(), PdfError> {
        let descendant = first_descendant_dictionary(self.document, font, self.limits)?;
        if let Some(descendant) = descendant {
            self.inspect_cid_subset_font(descendant, object_id, description)?;
        }
        if let Some(descendant) = descendant
            && resolved_name(self.document, descendant, b"Subtype", self.limits)?
                == Some(b"CIDFontType2".as_slice())
            && font_is_embedded(self.document, descendant, self.limits)?
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
        // Confirmed live against veraPDF 1.30.2: an *indirect* reference to
        // the name /Identity-H or /Identity-V is accepted exactly like a
        // direct one, so resolution must happen before checking either
        // shape, not only before the stream fallback.
        let resolved = resolve_optional(self.document, encoding, self.limits.max_reference_depth)?;
        if resolved
            .and_then(|object| object.as_name().ok())
            .is_some_and(|name| matches!(name, b"Identity-H" | b"Identity-V"))
        {
            return Ok(());
        }
        let Some(cmap) = resolved.and_then(|object| object.as_stream().ok()) else {
            let failure = font_failure(
                object_id,
                description,
                "uses a non-Identity CMap that is not embedded",
            );
            if resolved
                .and_then(|object| object.as_name().ok())
                .is_some_and(is_pdfa_2_3_predefined_cmap)
            {
                self.unembedded_predefined_cmaps.push(failure);
            } else {
                self.unembedded_cmaps.push(failure);
            }
            if let Some(descendant) = descendant
                && let Some(name) = resolved.and_then(|object| object.as_name().ok())
                && let Some(bytes) = predefined_cmaps::get(name)
            {
                self.record_type0_system_info(
                    object_id,
                    description,
                    cid_system_info(self.document, descendant, self.limits)?,
                    cmap_bytes_system_info(bytes),
                );
            }
            return Ok(());
        };

        let cmap_name = resolved_name(self.document, &cmap.dict, b"CMapName", self.limits)?;
        if cmap_name.is_some_and(|name| matches!(name, b"Identity-H" | b"Identity-V")) {
            return Ok(());
        }

        if let Some(descendant) = descendant {
            self.record_type0_system_info(
                object_id,
                description,
                cid_system_info(self.document, descendant, self.limits)?,
                cid_system_info(self.document, &cmap.dict, self.limits)?,
            );
        }

        let dictionary_wmode = cmap
            .dict
            .get(b"WMode")
            .ok()
            .map(|value| resolved_integer(self.document, self.limits, value))
            .transpose()?
            .flatten()
            .unwrap_or(0);
        let bytes = decode_font_stream(cmap, self.limits)?;
        let dictionary_reference_invalid =
            resolved_name(self.document, &cmap.dict, b"UseCMap", self.limits)?
                .is_some_and(|name| !is_pdfa_2_3_predefined_cmap(name));
        let embedded_reference_invalid = cmap_usecmap_name(&bytes)
            .is_some_and(|base_name| !is_pdfa_2_3_predefined_cmap(base_name));
        if dictionary_reference_invalid || embedded_reference_invalid {
            self.invalid_cmap_references.push(font_failure(
                object_id,
                description,
                "references a CMap outside the PDF/A-2 and PDF/A-3 predefined CMap set",
            ));
        }
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

    fn record_type0_system_info(
        &mut self,
        object_id: Option<PdfObjectId>,
        description: &str,
        cid_font: Option<CidSystemInfo>,
        cmap: Option<CidSystemInfo>,
    ) {
        let pdfa1_matches = cid_font
            .as_ref()
            .zip(cmap.as_ref())
            .is_some_and(|(cid_font, cmap)| cid_font.matches_pdfa1(cmap));
        if !pdfa1_matches {
            self.incompatible_type0_system_info.push(font_failure(
                object_id,
                description,
                "has incompatible CIDSystemInfo Registry or Ordering values in its CIDFont and CMap",
            ));
        }
        let pdfa2_matches = cid_font
            .as_ref()
            .zip(cmap.as_ref())
            .is_some_and(|(cid_font, cmap)| cid_font.matches_pdfa2_or_3(cmap));
        if !pdfa2_matches {
            self.incompatible_type0_system_info_pdfa2
                .push(font_failure(
                    object_id,
                    description,
                    "has incompatible CIDSystemInfo Registry, Ordering, or Supplement values in its CIDFont and CMap",
                ));
        }
    }

    fn inspect_cid_subset_font(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
    ) -> Result<(), PdfError> {
        let is_subset = resolved_name(self.document, font, b"BaseFont", self.limits)?
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
        if resolved_name(self.document, font, b"Type", self.limits)? != Some(b"Font".as_slice()) {
            self.invalid_types.push(font_failure(
                object_id,
                description,
                "has a missing or invalid /Type instead of /Font",
            ));
        }

        let base_font = resolved_name(self.document, font, b"BaseFont", self.limits)?;
        if subtype != Some("Type3") && base_font.is_none() {
            self.invalid_base_fonts.push(font_failure(
                object_id,
                description,
                "has a missing or invalid /BaseFont",
            ));
        }

        // Confirmed live against veraPDF 1.30.2: the "standard 14 fonts"
        // /FirstChar//LastChar//Widths exemption applies only to Type1/
        // MMType1 -- a TrueType (or Type3) font whose /BaseFont happens to
        // match a standard-14 name (e.g. "Helvetica") is still required to
        // supply all three, since standard-14 metrics are a Type1 concept.
        let requires_simple_font_metrics = match subtype {
            Some("Type1" | "MMType1") => !base_font.is_some_and(is_standard_14_font),
            Some("TrueType" | "Type3") => true,
            _ => false,
        };
        if requires_simple_font_metrics {
            let first_char = font
                .get(b"FirstChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
            let last_char = font
                .get(b"LastChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
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
            let widths_size = resolved_array(self.document, font, b"Widths", self.limits)?
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

        if let Some((invalid_subtype, valid_pdfa2)) =
            invalid_embedded_font_subtype(self.document, font, self.limits)?
        {
            self.invalid_font_file_subtypes.push(font_failure(
                object_id,
                description,
                &format!("uses unsupported embedded font subtype /{invalid_subtype}"),
            ));
            if !valid_pdfa2 {
                self.invalid_font_file_subtypes_pdfa2.push(font_failure(
                    object_id,
                    description,
                    &format!("uses unsupported embedded font subtype /{invalid_subtype}"),
                ));
            }
        }
        Ok(())
    }
}

struct SimpleFontEncoding {
    base: Option<PredefinedEncoding>,
    differences: BTreeMap<u8, String>,
}

impl SimpleFontEncoding {
    fn glyph_name(&self, byte: u8) -> Option<&str> {
        self.differences.get(&byte).map(String::as_str).or_else(|| {
            self.base
                .and_then(|base| font_encodings::glyph_name(base, byte))
        })
    }
}

fn simple_font_encoding(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<SimpleFontEncoding, PdfError> {
    let Ok(encoding) = font.get(b"Encoding") else {
        return Ok(SimpleFontEncoding {
            base: None,
            differences: BTreeMap::new(),
        });
    };
    let Some(encoding) = resolve_optional(document, encoding, limits.max_reference_depth)? else {
        return Ok(SimpleFontEncoding {
            base: None,
            differences: BTreeMap::new(),
        });
    };
    if let Ok(name) = encoding.as_name() {
        return Ok(SimpleFontEncoding {
            base: PredefinedEncoding::from_pdf_name(name),
            differences: BTreeMap::new(),
        });
    }
    let Ok(encoding) = encoding.as_dict() else {
        return Ok(SimpleFontEncoding {
            base: None,
            differences: BTreeMap::new(),
        });
    };
    let base = resolved_name(document, encoding, b"BaseEncoding", limits)?
        .and_then(PredefinedEncoding::from_pdf_name);
    let Some(differences) = encoding
        .get(b"Differences")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_array().ok())
    else {
        return Ok(SimpleFontEncoding {
            base,
            differences: BTreeMap::new(),
        });
    };
    let mut names = BTreeMap::new();
    let mut code: Option<i64> = None;
    for entry in differences {
        if let Some(value) = resolved_integer(document, limits, entry)? {
            code = Some(value);
        } else if let (Some(current), Some(name)) = (
            code,
            resolve_optional(document, entry, limits.max_reference_depth)?
                .and_then(|entry| entry.as_name().ok()),
        ) {
            if let Ok(current) = u8::try_from(current) {
                names.insert(current, String::from_utf8_lossy(name).into_owned());
            }
            code = current.checked_add(1);
        }
    }
    Ok(SimpleFontEncoding {
        base,
        differences: names,
    })
}

/// A `/CIDToGIDMap` resolved once per font instead of once per rendered
/// CID, since the map itself (`Identity` or a decoded stream) never changes
/// across the many CIDs a single font renders.
enum CidToGidMap {
    Identity,
    Table(Vec<u8>),
    Unavailable,
}

impl CidToGidMap {
    fn glyph_for(&self, cid: u16) -> Option<ttf_parser::GlyphId> {
        match self {
            Self::Identity => Some(ttf_parser::GlyphId(cid)),
            Self::Table(bytes) => {
                let offset = usize::from(cid).checked_mul(2)?;
                let entry = bytes.get(offset..offset.saturating_add(2))?;
                Some(ttf_parser::GlyphId(u16::from_be_bytes([
                    entry[0], entry[1],
                ])))
            }
            Self::Unavailable => None,
        }
    }
}

fn resolve_cid_to_gid_map(
    document: &Document,
    map: &Object,
    limits: &SafetyLimits,
) -> Result<CidToGidMap, PdfError> {
    // Confirmed live against veraPDF 1.30.2 (matching `valid_cid_to_gid_map`):
    // an indirect reference to the name /Identity is accepted exactly like
    // a direct one, so resolution must happen before checking either shape.
    let resolved = resolve_optional(document, map, limits.max_reference_depth)?;
    if resolved.and_then(|value| value.as_name().ok()) == Some(b"Identity".as_slice()) {
        return Ok(CidToGidMap::Identity);
    }
    let Some(stream) = resolved.and_then(|value| value.as_stream().ok()) else {
        return Ok(CidToGidMap::Unavailable);
    };
    Ok(CidToGidMap::Table(decode_font_stream(stream, limits)?))
}

/// A single `/W` array group, in original array order.
enum WGroup {
    /// `c [w1 w2 ... wn]`: width for CID `first + i` is `widths[i]`, when
    /// present and numeric.
    Singles {
        first: u16,
        widths: Vec<Option<f64>>,
    },
    /// `cFirst cLast w`.
    Range { first: u16, last: u16, width: f64 },
}

/// A font's `/W` (and `/DW`) entries, parsed once instead of rescanning the
/// whole array for every rendered CID. `width_for` reproduces
/// the original sequential-scan predicate exactly, including that a
/// malformed group aborts the lookup for every CID from that point on
/// *without* falling back to `/DW` (unlike a fully well-formed array that
/// simply has no matching group).
struct CidWidths {
    groups: Vec<WGroup>,
    truncated: bool,
    default_width: Option<f64>,
}

impl CidWidths {
    fn width_for(&self, cid: u16) -> Option<f64> {
        for group in &self.groups {
            match group {
                WGroup::Singles { first, widths } => {
                    if let Some(offset) = cid.checked_sub(*first)
                        && let Some(width) = widths.get(usize::from(offset))
                    {
                        return *width;
                    }
                }
                WGroup::Range { first, last, width } => {
                    if (*first..=*last).contains(&cid) {
                        return Some(*width);
                    }
                }
            }
        }
        if self.truncated {
            None
        } else {
            self.default_width
        }
    }
}

fn parse_cid_widths(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<CidWidths, PdfError> {
    let default_width = font
        .get(b"DW")
        .ok()
        .map(|value| resolved_float(document, limits, value))
        .transpose()?
        .flatten()
        .map(f64::from);
    let none = || CidWidths {
        groups: Vec::new(),
        truncated: false,
        default_width,
    };
    let Ok(value) = font.get(b"W") else {
        return Ok(none());
    };
    let Some(array) = resolve_optional(document, value, limits.max_reference_depth)?
        .and_then(|value| value.as_array().ok())
    else {
        return Ok(none());
    };

    let mut groups = Vec::new();
    let mut index = 0usize;
    let truncated = loop {
        if index >= array.len() {
            break false;
        }
        let Some(first) = array
            .get(index)
            .map(|value| resolved_integer(document, limits, value))
            .transpose()?
            .flatten()
            .and_then(|value| u16::try_from(value).ok())
        else {
            break true;
        };
        let Some(next) = array
            .get(index + 1)
            .map(|value| resolve_optional(document, value, limits.max_reference_depth))
            .transpose()?
            .flatten()
        else {
            break true;
        };
        if let Ok(widths) = next.as_array() {
            // The original scan converts each offset `0..widths.len()` to a
            // `u16` one at a time, aborting the moment that overflows.
            // Reaching offset 65536 is exactly the same overflow point.
            let usable_len = widths.len().min(usize::from(u16::MAX) + 1);
            let overflowed = widths.len() > usable_len;
            let mut resolved_widths = Vec::with_capacity(usable_len);
            for width in &widths[..usable_len] {
                resolved_widths.push(resolved_float(document, limits, width)?.map(f64::from));
            }
            groups.push(WGroup::Singles {
                first,
                widths: resolved_widths,
            });
            if overflowed {
                break true;
            }
            index += 2;
            continue;
        }
        let Some(last) = next
            .as_i64()
            .ok()
            .and_then(|value| u16::try_from(value).ok())
        else {
            break true;
        };
        let Some(width) = array
            .get(index + 2)
            .map(|value| resolved_float(document, limits, value))
            .transpose()?
            .flatten()
        else {
            break true;
        };
        groups.push(WGroup::Range {
            first,
            last,
            width: f64::from(width),
        });
        index += 3;
    };

    Ok(CidWidths {
        groups,
        truncated,
        default_width,
    })
}

fn single_encoded_character(encoding: &lopdf::Encoding<'_>, byte: u8) -> Option<char> {
    let value = encoding.bytes_to_string(&[byte]).ok()?;
    let mut characters = value.chars();
    let character = characters.next()?;
    characters.next().is_none().then_some(character)
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
    // Confirmed live against veraPDF 1.30.2: an *indirect* reference to the
    // name /Identity is accepted exactly like a direct one, so resolution
    // must happen before checking either shape, not only before the stream
    // fallback.
    let Some(resolved) = resolve_optional(document, value, limits.max_reference_depth)? else {
        return Ok(false);
    };
    Ok(resolved.as_name().ok() == Some(b"Identity".as_slice()) || resolved.as_stream().is_ok())
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
    // Confirmed live against veraPDF 1.30.2: an indirect reference to the
    // /Registry or /Ordering string is resolved exactly like a direct one.
    let Some(registry) = resolved_string(document, info, b"Registry", limits)? else {
        return Ok(None);
    };
    let Some(ordering) = resolved_string(document, info, b"Ordering", limits)? else {
        return Ok(None);
    };
    let supplement = info
        .get(b"Supplement")
        .ok()
        .map(|value| resolved_integer(document, limits, value))
        .transpose()?
        .flatten();
    Ok(Some(CidSystemInfo {
        registry,
        ordering,
        supplement,
    }))
}

fn resolved_string(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<Option<Vec<u8>>, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, value, limits.max_reference_depth)?.and_then(|object| {
            match object {
                Object::String(value, _) => Some(value.clone()),
                _ => None,
            }
        }),
    )
}

/// A dictionary array value (e.g. `/Widths`) resolved through indirection
/// before use, mirroring `resolved_string`/`resolved_integer` -- the array
/// itself is not guaranteed to be direct.
fn resolved_array<'a>(
    document: &'a Document,
    dictionary: &'a Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<Option<&'a Vec<Object>>, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, value, limits.max_reference_depth)?
            .and_then(|object| object.as_array().ok()),
    )
}

/// A dictionary name value (e.g. `/Subtype`) resolved through indirection
/// before use, mirroring `resolved_string`/`resolved_array`.
fn resolved_name<'a>(
    document: &'a Document,
    dictionary: &'a Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<Option<&'a [u8]>, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, value, limits.max_reference_depth)?
            .and_then(|object| object.as_name().ok()),
    )
}

/// A single numeric array element (e.g. a `/Widths` entry) resolved through
/// indirection before use, mirroring `resolved_integer`'s contract for a
/// real-valued rather than integer-valued reference.
fn resolved_float(
    document: &Document,
    limits: &SafetyLimits,
    object: &Object,
) -> Result<Option<f32>, PdfError> {
    Ok(
        resolve_optional(document, object, limits.max_reference_depth)?
            .and_then(|object| object.as_float().ok()),
    )
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

/// Clamps a CMap's self-declared entry count to the number of entries the
/// remaining token buffer could actually hold, so a tiny malicious stream
/// declaring an astronomical count (e.g. `99999999999999 begincidrange
/// <00> <01> 1 endcidrange`, ~50 bytes) cannot force a near-unbounded loop:
/// every claimed entry beyond the real buffer size would only ever find
/// missing tokens via `.get()` anyway, so clamping changes no outcome for a
/// well-formed or merely under-provisioned declaration.
fn bounded_cmap_entry_count(
    declared: usize,
    tokens: &[&[u8]],
    entries_start: usize,
    entry_width: usize,
) -> usize {
    declared.min(tokens.len().saturating_sub(entries_start) / entry_width)
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
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 2);
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
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 3);
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
            Some(b"beginnotdefchar") => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 2);
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
            Some(b"beginnotdefrange") => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 3);
                for entry in 0..count {
                    if let Some(cid) = tokens
                        .get(cursor + 4 + entry * 3)
                        .and_then(|token| parse_cmap_integer(token))
                    {
                        maximum = Some(maximum.unwrap_or(cid).max(cid));
                    }
                }
                cursor += 2 + count * 3;
            }
            _ => cursor += 1,
        }
    }
    maximum
}

/// A Type0 font's resolved `/Encoding`, cacheable per indirect CMap object
/// since it never changes across the many rendered-byte sequences checked
/// against it.
enum CmapDecoder {
    /// The literal name `/Identity-H`/`/Identity-V`, or an embedded CMap
    /// that itself `usecmap`s one of them: every two shown bytes decode
    /// directly as a big-endian CID.
    IdentityBytes,
    /// A parsed explicit embedded CMap.
    Parsed(ParsedCmap),
    /// `encoding` did not resolve to a usable CMap.
    Unavailable,
}

impl CmapDecoder {
    fn codes(&self, shown_bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
        match self {
            Self::IdentityBytes => shown_bytes.chunks_exact(2).next().is_some().then(|| {
                shown_bytes
                    .chunks_exact(2)
                    .map(|code| code.to_vec())
                    .collect()
            }),
            Self::Parsed(parsed) => parsed.codes(shown_bytes),
            Self::Unavailable => None,
        }
    }
}

impl CmapDecoder {
    fn decode(&self, shown_bytes: &[u8]) -> Option<Vec<u16>> {
        match self {
            Self::IdentityBytes => identity_cids(shown_bytes),
            Self::Parsed(parsed) => parsed.decode(shown_bytes),
            Self::Unavailable => None,
        }
    }
}

fn identity_cids(shown_bytes: &[u8]) -> Option<Vec<u16>> {
    let mut chunks = shown_bytes.chunks_exact(2);
    let cids = chunks
        .by_ref()
        .map(|cid| u16::from_be_bytes([cid[0], cid[1]]))
        .collect();
    chunks.remainder().is_empty().then_some(cids)
}

fn resolve_cmap_decoder(
    document: &Document,
    encoding: &Object,
    limits: &SafetyLimits,
) -> Result<CmapDecoder, PdfError> {
    let resolved = resolve_optional(document, encoding, limits.max_reference_depth)?;
    // Confirmed live against veraPDF 1.30.2: an indirect reference to the
    // name /Identity-H or /Identity-V is accepted exactly like a direct one.
    if let Some(name) = resolved.and_then(|object| object.as_name().ok()) {
        if matches!(name, b"Identity-H" | b"Identity-V") {
            return Ok(CmapDecoder::IdentityBytes);
        }
        return Ok(
            match predefined_cmaps::get(name).and_then(parse_cmap_with_predefined_bases) {
                Some(parsed) => CmapDecoder::Parsed(parsed),
                None => CmapDecoder::Unavailable,
            },
        );
    }
    let Some(cmap) = resolved.and_then(|object| object.as_stream().ok()) else {
        return Ok(CmapDecoder::Unavailable);
    };
    let bytes = decode_font_stream(cmap, limits)?;
    if cmap_uses_identity_base(&bytes) && !cmap_has_explicit_mappings(&bytes) {
        return Ok(CmapDecoder::IdentityBytes);
    }
    Ok(match parse_cmap_with_predefined_bases(&bytes) {
        Some(parsed) => CmapDecoder::Parsed(parsed),
        None => CmapDecoder::Unavailable,
    })
}

#[derive(Clone, Copy)]
struct CmapCodeSpace {
    bytes: usize,
    start: u32,
    end: u32,
}

#[derive(Clone, Copy)]
struct CmapCidRange {
    bytes: usize,
    start: u32,
    end: u32,
    first_cid: u16,
}

#[derive(Clone, Copy)]
struct CmapNotDefRange {
    bytes: usize,
    start: u32,
    end: u32,
    cid: u16,
}

fn cmap_uses_identity_base(bytes: &[u8]) -> bool {
    let tokens = cmap_tokens(bytes);
    tokens
        .windows(2)
        .filter_map(|pair| {
            (pair[1] == b"usecmap")
                .then(|| pair[0].strip_prefix(b"/"))
                .flatten()
        })
        .any(|name| matches!(name, b"Identity-H" | b"Identity-V"))
}

/// The complete predefined CMap set in ISO 32000-1:2008 Table 118, copied
/// from the pinned veraPDF 1.30.2 PDF/A-2 and PDF/A-3 profiles.
fn is_pdfa_2_3_predefined_cmap(name: &[u8]) -> bool {
    matches!(
        name,
        b"Identity-H"
            | b"Identity-V"
            | b"GB-EUC-H"
            | b"GB-EUC-V"
            | b"GBpc-EUC-H"
            | b"GBpc-EUC-V"
            | b"GBK-EUC-H"
            | b"GBK-EUC-V"
            | b"GBKp-EUC-H"
            | b"GBKp-EUC-V"
            | b"GBK2K-H"
            | b"GBK2K-V"
            | b"UniGB-UCS2-H"
            | b"UniGB-UCS2-V"
            | b"UniGB-UTF16-H"
            | b"UniGB-UTF16-V"
            | b"B5pc-H"
            | b"B5pc-V"
            | b"HKscs-B5-H"
            | b"HKscs-B5-V"
            | b"ETen-B5-H"
            | b"ETen-B5-V"
            | b"ETenms-B5-H"
            | b"ETenms-B5-V"
            | b"CNS-EUC-H"
            | b"CNS-EUC-V"
            | b"UniCNS-UCS2-H"
            | b"UniCNS-UCS2-V"
            | b"UniCNS-UTF16-H"
            | b"UniCNS-UTF16-V"
            | b"83pv-RKSJ-H"
            | b"90ms-RKSJ-H"
            | b"90ms-RKSJ-V"
            | b"90msp-RKSJ-H"
            | b"90msp-RKSJ-V"
            | b"90pv-RKSJ-H"
            | b"Add-RKSJ-H"
            | b"Add-RKSJ-V"
            | b"EUC-H"
            | b"EUC-V"
            | b"Ext-RKSJ-H"
            | b"Ext-RKSJ-V"
            | b"H"
            | b"V"
            | b"UniJIS-UCS2-H"
            | b"UniJIS-UCS2-V"
            | b"UniJIS-UCS2-HW-H"
            | b"UniJIS-UCS2-HW-V"
            | b"UniJIS-UTF16-H"
            | b"UniJIS-UTF16-V"
            | b"KSC-EUC-H"
            | b"KSC-EUC-V"
            | b"KSCms-UHC-H"
            | b"KSCms-UHC-V"
            | b"KSCms-UHC-HW-H"
            | b"KSCms-UHC-HW-V"
            | b"KSCpc-EUC-H"
            | b"UniKS-UCS2-H"
            | b"UniKS-UCS2-V"
            | b"UniKS-UTF16-H"
            | b"UniKS-UTF16-V"
    )
}

fn cmap_has_explicit_mappings(bytes: &[u8]) -> bool {
    cmap_tokens(bytes).iter().any(|token| {
        matches!(
            *token,
            b"begincidchar" | b"begincidrange" | b"beginnotdefchar" | b"beginnotdefrange"
        )
    })
}

/// An embedded CMap's code spaces, single-CID mappings, and CID ranges,
/// parsed once from its raw bytes and reused for every rendered-byte
/// sequence decoded through it.
struct ParsedCmap {
    code_spaces: Vec<CmapCodeSpace>,
    chars: BTreeMap<(usize, u32), u16>,
    ranges: Vec<CmapCidRange>,
    notdef_chars: BTreeMap<(usize, u32), u16>,
    notdef_ranges: Vec<CmapNotDefRange>,
}

/// Resolves explicit CID CMaps with one- through four-byte code spaces.
fn parse_cmap(bytes: &[u8]) -> Option<ParsedCmap> {
    let tokens = cmap_tokens(bytes);
    let mut cursor = 0usize;
    let mut code_spaces = Vec::new();
    let mut chars = BTreeMap::new();
    let mut ranges = Vec::new();
    let mut notdef_chars = BTreeMap::new();
    let mut notdef_ranges = Vec::new();
    while cursor + 1 < tokens.len() {
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
        match tokens[cursor + 1] {
            b"begincodespacerange" => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 2);
                for entry in 0..count {
                    let base = cursor + 2 + entry * 2;
                    let (Some(start), Some(end)) = (
                        tokens.get(base).and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 1)
                            .and_then(|token| parse_cmap_code(token)),
                    ) else {
                        continue;
                    };
                    if start.0 == end.0 && start.1 <= end.1 {
                        code_spaces.push(CmapCodeSpace {
                            bytes: start.0,
                            start: start.1,
                            end: end.1,
                        });
                    }
                }
                cursor += 2 + count * 2;
            }
            b"begincidchar" => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 2);
                for entry in 0..count {
                    let base = cursor + 2 + entry * 2;
                    let (Some(code), Some(cid)) = (
                        tokens.get(base).and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 1)
                            .and_then(|token| parse_cmap_integer(token)),
                    ) else {
                        continue;
                    };
                    if cid <= 65_535 {
                        chars.insert(code, cid as u16);
                    }
                }
                cursor += 2 + count * 2;
            }
            b"begincidrange" => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 3);
                for entry in 0..count {
                    let base = cursor + 2 + entry * 3;
                    let (Some(start), Some(end), Some(cid)) = (
                        tokens.get(base).and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 1)
                            .and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 2)
                            .and_then(|token| parse_cmap_integer(token)),
                    ) else {
                        continue;
                    };
                    if start.0 == end.0
                        && start.1 <= end.1
                        && cid.saturating_add(end.1 - start.1) <= 65_535
                    {
                        ranges.push(CmapCidRange {
                            bytes: start.0,
                            start: start.1,
                            end: end.1,
                            first_cid: cid as u16,
                        });
                    }
                }
                cursor += 2 + count * 3;
            }
            b"beginnotdefchar" => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 2);
                for entry in 0..count {
                    let base = cursor + 2 + entry * 2;
                    let (Some(code), Some(cid)) = (
                        tokens.get(base).and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 1)
                            .and_then(|token| parse_cmap_integer(token)),
                    ) else {
                        continue;
                    };
                    if cid <= 65_535 {
                        notdef_chars.insert(code, cid as u16);
                    }
                }
                cursor += 2 + count * 2;
            }
            b"beginnotdefrange" => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 3);
                for entry in 0..count {
                    let base = cursor + 2 + entry * 3;
                    let (Some(start), Some(end), Some(cid)) = (
                        tokens.get(base).and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 1)
                            .and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 2)
                            .and_then(|token| parse_cmap_integer(token)),
                    ) else {
                        continue;
                    };
                    if start.0 == end.0 && start.1 <= end.1 && cid <= 65_535 {
                        notdef_ranges.push(CmapNotDefRange {
                            bytes: start.0,
                            start: start.1,
                            end: end.1,
                            cid: cid as u16,
                        });
                    }
                }
                cursor += 2 + count * 3;
            }
            _ => cursor += 1,
        }
    }
    if code_spaces.is_empty()
        && chars.is_empty()
        && ranges.is_empty()
        && notdef_chars.is_empty()
        && notdef_ranges.is_empty()
    {
        return None;
    }
    code_spaces.sort_by_key(|space| space.bytes);
    Some(ParsedCmap {
        code_spaces,
        chars,
        ranges,
        notdef_chars,
        notdef_ranges,
    })
}

fn parse_cmap_with_predefined_bases(bytes: &[u8]) -> Option<ParsedCmap> {
    parse_cmap_with_bases(bytes, &mut BTreeSet::new(), 0)
}

fn parse_cmap_with_bases(
    bytes: &[u8],
    active_names: &mut BTreeSet<Vec<u8>>,
    depth: usize,
) -> Option<ParsedCmap> {
    const MAX_PREDEFINED_CMAP_DEPTH: usize = 16;
    if depth > MAX_PREDEFINED_CMAP_DEPTH {
        return None;
    }

    let parsed = parse_cmap(bytes);
    let Some(base_name) = cmap_usecmap_name(bytes) else {
        return parsed;
    };
    if matches!(base_name, b"Identity-H" | b"Identity-V") {
        let base = identity_parsed_cmap();
        return match parsed {
            Some(mut parsed) => {
                parsed.inherit(base);
                Some(parsed)
            }
            None => Some(base),
        };
    }
    if !active_names.insert(base_name.to_vec()) {
        return None;
    }
    let base = predefined_cmaps::get(base_name)
        .and_then(|base| parse_cmap_with_bases(base, active_names, depth + 1));
    active_names.remove(base_name);
    match (parsed, base) {
        (Some(mut parsed), Some(base)) => {
            parsed.inherit(base);
            Some(parsed)
        }
        (None, base) => base,
        (Some(parsed), None) => Some(parsed),
    }
}

fn identity_parsed_cmap() -> ParsedCmap {
    ParsedCmap {
        code_spaces: vec![CmapCodeSpace {
            bytes: 2,
            start: 0,
            end: u32::from(u16::MAX),
        }],
        chars: BTreeMap::new(),
        ranges: vec![CmapCidRange {
            bytes: 2,
            start: 0,
            end: u32::from(u16::MAX),
            first_cid: 0,
        }],
        notdef_chars: BTreeMap::new(),
        notdef_ranges: Vec::new(),
    }
}

fn cmap_usecmap_name(bytes: &[u8]) -> Option<&[u8]> {
    cmap_tokens(bytes).windows(2).find_map(|pair| {
        (pair[1] == b"usecmap")
            .then(|| pair[0].strip_prefix(b"/"))
            .flatten()
    })
}

fn cmap_bytes_system_info(bytes: &[u8]) -> Option<CidSystemInfo> {
    let tokens = cmap_tokens(bytes);
    let value = |key: &[u8]| {
        tokens.windows(2).find_map(|pair| {
            (pair[0] == key)
                .then(|| pair[1].strip_prefix(b"(")?.strip_suffix(b")"))
                .flatten()
        })
    };
    let supplement = tokens.windows(2).find_map(|pair| {
        (pair[0] == b"/Supplement")
            .then(|| parse_cmap_integer(pair[1]))
            .flatten()
    })?;
    Some(CidSystemInfo {
        registry: value(b"/Registry")?.to_vec(),
        ordering: value(b"/Ordering")?.to_vec(),
        supplement: Some(i64::from(supplement)),
    })
}

impl ParsedCmap {
    fn inherit(&mut self, base: Self) {
        if self.code_spaces.is_empty() {
            self.code_spaces = base.code_spaces;
        } else {
            self.code_spaces.extend(base.code_spaces);
            self.code_spaces
                .sort_by_key(|space| (space.bytes, space.start, space.end));
            self.code_spaces
                .dedup_by_key(|space| (space.bytes, space.start, space.end));
        }
        for (code, cid) in base.chars {
            self.chars.entry(code).or_insert(cid);
        }
        self.ranges.extend(base.ranges);
        for (code, cid) in base.notdef_chars {
            self.notdef_chars.entry(code).or_insert(cid);
        }
        self.notdef_ranges.extend(base.notdef_ranges);
    }

    fn decode(&self, shown_bytes: &[u8]) -> Option<Vec<u16>> {
        let mut cids = Vec::new();
        let mut position = 0usize;
        while position < shown_bytes.len() {
            let mut decoded = None;
            for space in &self.code_spaces {
                let Some(end) = position.checked_add(space.bytes) else {
                    continue;
                };
                let Some(code_bytes) = shown_bytes.get(position..end) else {
                    continue;
                };
                let code = code_bytes
                    .iter()
                    .fold(0_u32, |value, byte| value << 8 | u32::from(*byte));
                if code < space.start || code > space.end {
                    continue;
                }
                let cid = self
                    .chars
                    .get(&(space.bytes, code))
                    .copied()
                    .or_else(|| {
                        self.ranges
                            .iter()
                            .find(|range| {
                                range.bytes == space.bytes
                                    && range.start <= code
                                    && code <= range.end
                            })
                            .map(|range| range.first_cid + (code - range.start) as u16)
                    })
                    .or_else(|| {
                        self.notdef_chars
                            .get(&(space.bytes, code))
                            .copied()
                            .or_else(|| {
                                self.notdef_ranges
                                    .iter()
                                    .find(|range| {
                                        range.bytes == space.bytes
                                            && range.start <= code
                                            && code <= range.end
                                    })
                                    .map(|range| range.cid)
                            })
                    });
                decoded = Some((space.bytes, cid));
                break;
            }
            let Some((width, Some(cid))) = decoded else {
                return None;
            };
            position += width;
            cids.push(cid);
        }
        Some(cids)
    }

    fn codes(&self, shown_bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
        let mut codes = Vec::new();
        let mut position = 0;
        while position < shown_bytes.len() {
            let mut matched = None;
            for space in &self.code_spaces {
                let end = position.checked_add(space.bytes)?;
                let bytes = shown_bytes.get(position..end)?;
                let code = bytes
                    .iter()
                    .fold(0_u32, |value, byte| value << 8 | u32::from(*byte));
                if (space.start..=space.end).contains(&code) {
                    matched = Some(bytes.to_vec());
                    break;
                }
            }
            let code = matched?;
            position += code.len();
            codes.push(code);
        }
        Some(codes)
    }
}

fn unicode_mapping_exception(
    document: &Document,
    font: &Dictionary,
    subtype: Option<&str>,
    shown_bytes: &[u8],
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    if matches!(subtype, Some("Type1" | "MMType1" | "TrueType" | "Type3")) {
        let encoding = simple_font_encoding(document, font, limits)?;
        let standard = encoding.base == Some(PredefinedEncoding::MacRoman)
            || encoding.base == Some(PredefinedEncoding::MacExpert)
            || encoding.base == Some(PredefinedEncoding::WinAnsi);
        if standard && encoding.differences.is_empty() {
            return Ok(true);
        }
        if matches!(subtype, Some("Type1" | "MMType1")) {
            let standard_names = (0..=u8::MAX)
                .filter_map(|byte| font_encodings::glyph_name(PredefinedEncoding::Standard, byte))
                .collect::<BTreeSet<_>>();
            let all_standard = shown_bytes.iter().all(|byte| {
                encoding
                    .glyph_name(*byte)
                    .or_else(|| font_encodings::glyph_name(PredefinedEncoding::Standard, *byte))
                    .is_some_and(|name| standard_names.contains(name))
            });
            let symbol_font = resolved_name(document, font, b"BaseFont", limits)?
                .is_some_and(|name| name == b"Symbol");
            if all_standard || symbol_font {
                return Ok(true);
            }
        }
    }
    if subtype == Some("Type0") {
        if let Some(encoding) = font.get(b"Encoding").ok()
            && resolve_optional(document, encoding, limits.max_reference_depth)?
                .and_then(|object| object.as_name().ok())
                .is_some_and(|name| matches!(name, b"Identity-H" | b"Identity-V"))
        {
            return Ok(true);
        }
        if let Some(descendant) = first_descendant_dictionary(document, font, limits)?
            && let Some(system_info) = cid_system_info(document, descendant, limits)?
            && system_info.registry == b"Adobe"
            && matches!(
                system_info.ordering.as_slice(),
                b"GB1" | b"CNS1" | b"Japan1" | b"Korea1"
            )
        {
            return Ok(true);
        }
    }
    Ok(false)
}

struct UnicodeCmap {
    mappings: BTreeMap<Vec<u8>, Vec<u8>>,
    has_reserved_values: bool,
}

impl UnicodeCmap {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let tokens = cmap_tokens(bytes);
        let mut mappings = BTreeMap::new();
        let mut cursor = 0;
        while cursor + 1 < tokens.len() {
            let Some(count) = parse_cmap_integer(tokens[cursor]).map(|count| count as usize) else {
                cursor += 1;
                continue;
            };
            match tokens[cursor + 1] {
                b"beginbfchar" => {
                    for index in 0..count {
                        let source = parse_cmap_bytes(tokens.get(cursor + 2 + index * 2)?)?;
                        let destination = parse_cmap_bytes(tokens.get(cursor + 3 + index * 2)?)?;
                        mappings.insert(source, destination);
                    }
                    cursor += 2 + count * 2;
                }
                b"beginbfrange" => {
                    for index in 0..count {
                        let base = cursor + 2 + index * 3;
                        let start = parse_cmap_bytes(tokens.get(base)?)?;
                        let end = parse_cmap_bytes(tokens.get(base + 1)?)?;
                        let first = parse_cmap_bytes(tokens.get(base + 2)?)?;
                        if start.len() != end.len() || start.len() != first.len() || start > end {
                            return None;
                        }
                        let start_value = bytes_value(&start);
                        let end_value = bytes_value(&end);
                        let first_value = bytes_value(&first);
                        let range_length = end_value - start_value;
                        if range_length > 65_535 {
                            return None;
                        }
                        for offset in 0..=range_length {
                            let source = value_bytes(start_value + offset, start.len());
                            let destination = value_bytes(first_value + offset, first.len());
                            mappings.insert(source, destination);
                        }
                    }
                    cursor += 2 + count * 3;
                }
                _ => cursor += 1,
            }
        }
        (!mappings.is_empty() && mappings.values().all(|value| valid_unicode_bytes(value)))
            .then_some(Self {
                has_reserved_values: mappings.values().any(|value| {
                    value.chunks_exact(2).any(|pair| {
                        matches!(u16::from_be_bytes([pair[0], pair[1]]), 0 | 0xFEFF | 0xFFFE)
                    })
                }),
                mappings,
            })
    }

    fn maps_usable(&self, code: &[u8]) -> bool {
        self.mappings
            .get(code)
            .is_some_and(|value| valid_unicode_bytes(value))
    }

    fn maps_pua(&self, code: &[u8]) -> bool {
        self.mappings.get(code).is_some_and(|value| {
            String::from_utf16(
                &value
                    .chunks_exact(2)
                    .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                    .collect::<Vec<_>>(),
            )
            .is_ok_and(|value| {
                value.chars().any(|character| {
                    matches!(
                        character as u32,
                        0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD
                    )
                })
            })
        })
    }
}

fn parse_cmap_bytes(token: &[u8]) -> Option<Vec<u8>> {
    let value = token.strip_prefix(b"<")?.strip_suffix(b">")?;
    (value.len() % 2 == 0 && !value.is_empty()).then(|| {
        (0..value.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(std::str::from_utf8(&value[i..i + 2]).ok()?, 16).ok())
            .collect::<Option<Vec<_>>>()
    })?
}

fn bytes_value(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0, |value, byte| value << 8 | u32::from(*byte))
}

fn value_bytes(value: u32, length: usize) -> Vec<u8> {
    (0..length)
        .rev()
        .map(|shift| (value >> (shift * 8)) as u8)
        .collect()
}

fn valid_unicode_bytes(bytes: &[u8]) -> bool {
    bytes.len().is_multiple_of(2)
        && !bytes.is_empty()
        && String::from_utf16(
            &bytes
                .chunks_exact(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>(),
        )
        .is_ok()
}

fn cmap_tokens(bytes: &[u8]) -> Vec<&[u8]> {
    bytes
        .split(|byte| matches!(*byte, b'\r' | b'\n'))
        .flat_map(|line| line.split(|byte| *byte == b'%').next())
        .flat_map(|line| line.split(|byte| byte.is_ascii_whitespace()))
        .filter(|token| !token.is_empty())
        .collect()
}

fn parse_cmap_integer(token: &[u8]) -> Option<u32> {
    std::str::from_utf8(token).ok()?.parse().ok()
}

fn parse_cmap_code(token: &[u8]) -> Option<(usize, u32)> {
    let token = token.strip_prefix(b"<")?.strip_suffix(b">")?;
    if token.is_empty() || token.len() % 2 != 0 || token.len() > 8 {
        return None;
    }
    let value = std::str::from_utf8(token)
        .ok()
        .and_then(|token| u32::from_str_radix(token, 16).ok())?;
    Some((token.len() / 2, value))
}

fn parse_cmap_hex(token: &[u8]) -> Option<u32> {
    parse_cmap_code(token).map(|(_, value)| value)
}

fn is_subset_font_name(name: &[u8]) -> bool {
    name.len() >= 7 && name[..6].iter().all(u8::is_ascii_uppercase) && name[6] == b'+'
}

fn cid_set_contains(bytes: &[u8], cid: u16) -> bool {
    bytes
        .get(usize::from(cid) / 8)
        .is_some_and(|byte| byte & (1 << (7 - cid % 8)) != 0)
}

fn type1_charset_names(bytes: &[u8]) -> BTreeSet<&str> {
    std::str::from_utf8(bytes)
        .ok()
        .into_iter()
        .flat_map(|value| value.split('/').skip(1))
        .filter_map(|value| value.split_ascii_whitespace().next())
        .collect()
}

/// Decrypts Adobe Type 1 eexec/charstring ciphertext (Type 1 Font Format
/// spec §7): `initial_state` is 55665 for the outer eexec layer and 4330 for
/// individual charstrings; `skip` discards the corresponding lenIV-controlled
/// random leading bytes (conventionally 4 for eexec, `lenIV` for charstrings).
fn type1_decrypt(bytes: &[u8], initial_state: u16, skip: usize) -> Vec<u8> {
    let mut state = initial_state;
    bytes
        .iter()
        .copied()
        .map(|ciphertext| {
            let plaintext = ciphertext ^ (state >> 8) as u8;
            state = state
                .wrapping_add(u16::from(ciphertext))
                .wrapping_mul(52_845)
                .wrapping_add(22_719);
            plaintext
        })
        .skip(skip)
        .collect()
}

fn type1_program_char_names(bytes: &[u8]) -> BTreeSet<String> {
    let bytes = type1_pfb_payload(bytes);
    let Some(eexec) = bytes.windows(5).position(|window| window == b"eexec") else {
        return BTreeSet::new();
    };
    let ciphertext = type1_eexec_ciphertext(&bytes[eexec + 5..]);
    let plaintext = type1_decrypt(&ciphertext, 55_665, 4);
    let Some(start) = plaintext
        .windows(b"/CharStrings".len())
        .position(|window| window == b"/CharStrings")
    else {
        return BTreeSet::new();
    };
    plaintext[start..]
        .split(|byte| *byte == b'/')
        .skip(1)
        .filter_map(|entry| {
            let name = entry
                .iter()
                .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'.')
                .copied()
                .collect::<Vec<_>>();
            (!name.is_empty()).then(|| String::from_utf8_lossy(&name).into_owned())
        })
        .collect()
}

struct Type1ProgramEncoding {
    names: BTreeMap<u8, String>,
    standard_base: bool,
}

impl Type1ProgramEncoding {
    fn glyph_name(&self, byte: u8) -> Option<&str> {
        self.names.get(&byte).map(String::as_str).or_else(|| {
            self.standard_base
                .then(|| font_encodings::glyph_name(PredefinedEncoding::Standard, byte))
                .flatten()
        })
    }
}

/// Parses the clear-text Type 1 `/Encoding` definition before `eexec`.
/// Custom array encodings are expressed as bounded `dup code /name put`
/// assignments; a copied `StandardEncoding` supplies the base before those
/// overrides. A missing/unreadable declaration remains unmapped, matching
/// veraPDF's zero-filled 256-entry program encoding array.
fn type1_program_encoding(bytes: &[u8]) -> Type1ProgramEncoding {
    let bytes = type1_pfb_payload(bytes);
    let clear = bytes
        .windows(5)
        .position(|window| window == b"eexec")
        .and_then(|position| bytes.get(..position))
        .unwrap_or(&bytes);
    let tokens = clear
        .split(|byte| matches!(*byte, b'\r' | b'\n'))
        .flat_map(|line| line.split(|byte| *byte == b'%').next())
        .flat_map(|line| line.split(u8::is_ascii_whitespace))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let Some(start) = tokens.iter().position(|token| *token == b"/Encoding") else {
        return Type1ProgramEncoding {
            names: BTreeMap::new(),
            standard_base: false,
        };
    };
    let definition = &tokens[start + 1..];
    let end = definition
        .iter()
        .position(|token| *token == b"def")
        .unwrap_or(definition.len());
    let definition = &definition[..end];
    let standard_base = definition.contains(&b"StandardEncoding".as_slice());
    let mut names = BTreeMap::new();
    for assignment in definition.windows(4) {
        if assignment[0] != b"dup" || assignment[3] != b"put" {
            continue;
        }
        let Some(code) = std::str::from_utf8(assignment[1])
            .ok()
            .and_then(|value| value.parse::<u8>().ok())
        else {
            continue;
        };
        let Some(name) = assignment[2].strip_prefix(b"/") else {
            continue;
        };
        if !name.is_empty() {
            names.insert(code, String::from_utf8_lossy(name).into_owned());
        }
    }
    Type1ProgramEncoding {
        names,
        standard_base,
    }
}

fn type1_program_charstring_widths(bytes: &[u8]) -> BTreeMap<String, f64> {
    let bytes = type1_pfb_payload(bytes);
    let Some(eexec) = bytes.windows(5).position(|window| window == b"eexec") else {
        return BTreeMap::new();
    };
    let ciphertext = type1_eexec_ciphertext(&bytes[eexec + 5..]);
    let plaintext = type1_decrypt(&ciphertext, 55_665, 4);
    let len_iv = plaintext
        .windows(b"/lenIV".len())
        .position(|window| window == b"/lenIV")
        .and_then(|position| parse_ascii_integer(&plaintext[position + 6..]))
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(4);
    let Some(charstrings) = plaintext
        .windows(b"/CharStrings".len())
        .position(|window| window == b"/CharStrings")
    else {
        return BTreeMap::new();
    };
    let mut result = BTreeMap::new();
    let mut position = charstrings + b"/CharStrings".len();
    while let Some(relative) = plaintext[position..].iter().position(|byte| *byte == b'/') {
        position += relative + 1;
        let name_start = position;
        while plaintext
            .get(position)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'.')
        {
            position += 1;
        }
        if position == name_start {
            continue;
        }
        let name = String::from_utf8_lossy(&plaintext[name_start..position]).into_owned();
        while plaintext
            .get(position)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            position += 1;
        }
        let Some(length) = parse_ascii_integer(&plaintext[position..])
            .and_then(|value| usize::try_from(value).ok())
        else {
            break;
        };
        while plaintext
            .get(position)
            .is_some_and(|byte| !byte.is_ascii_whitespace())
        {
            position += 1;
        }
        while plaintext
            .get(position)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            position += 1;
        }
        if plaintext.get(position..position + 2) != Some(b"RD") {
            continue;
        }
        position += 2;
        while plaintext
            .get(position)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            position += 1;
        }
        let Some(end) = position.checked_add(length) else {
            break;
        };
        let Some(charstring) = plaintext.get(position..end) else {
            break;
        };
        if let Some(width) = type1_charstring_width(charstring, len_iv) {
            result.insert(name, width);
        }
        position = end;
    }
    result
}

fn type1_charstring_width(bytes: &[u8], len_iv: usize) -> Option<f64> {
    let decrypted = type1_decrypt(bytes, 4_330, len_iv);
    let mut operands = Vec::new();
    let mut position = 0;
    while position < decrypted.len() {
        if let Some((value, consumed)) = cff_number(&decrypted[position..]) {
            operands.push(value);
            position += consumed;
            continue;
        }
        let byte = decrypted[position];
        if byte == 13 && operands.len() >= 2 {
            return operands.get(1).copied();
        }
        if byte == 12 && decrypted.get(position + 1) == Some(&7) && operands.len() >= 3 {
            return operands.get(2).copied();
        }
        return None;
    }
    None
}

fn parse_ascii_integer(bytes: &[u8]) -> Option<i64> {
    let mut end = 0;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_whitespace() || byte.is_ascii_digit() || *byte == b'-')
    {
        end += 1;
    }
    std::str::from_utf8(&bytes[..end]).ok()?.trim().parse().ok()
}

fn type1_eexec_ciphertext(bytes: &[u8]) -> Vec<u8> {
    let bytes = bytes
        .iter()
        .skip_while(|byte| byte.is_ascii_whitespace())
        .copied()
        .collect::<Vec<_>>();
    let ascii_hex = bytes
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .take(4)
        .all(|byte| byte.is_ascii_hexdigit());
    if ascii_hex {
        let hex = bytes
            .iter()
            .copied()
            .take_while(|byte| byte.is_ascii_whitespace() || byte.is_ascii_hexdigit())
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect::<Vec<_>>();
        return hex
            .chunks_exact(2)
            .map(|pair| {
                let digit = |byte| match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    b'A'..=b'F' => byte - b'A' + 10,
                    _ => unreachable!("checked ASCII hexadecimal byte"),
                };
                digit(pair[0]) << 4 | digit(pair[1])
            })
            .collect();
    }
    bytes
}

fn cff_cid_glyph_width(bytes: &[u8], glyph_id: u16) -> Option<f64> {
    let (_, top_offset) = cff_index(bytes, 4)?;
    let (top_items, _) = cff_index(bytes, top_offset)?;
    let top = top_items.first().copied()?;
    let top_dict = cff_dict(top);
    let charstrings_offset = cff_dict_offset(&top_dict, 17)?;
    let (charstrings, _) = cff_index(bytes, charstrings_offset)?;
    let charstring = charstrings.get(usize::from(glyph_id))?;
    let fd_array_offset = cff_dict_offset(&top_dict, 1236)?;
    let fd_select_offset = cff_dict_offset(&top_dict, 1237)?;
    let fd_index = cff_fd_select(bytes, fd_select_offset, glyph_id)?;
    let (fd_array, _) = cff_index(bytes, fd_array_offset)?;
    let fd_dict = cff_dict(fd_array.get(usize::from(fd_index))?);
    let private_values = fd_dict.get(&18)?;
    let private_size = usize::try_from(private_values.first().copied()? as i64).ok()?;
    let private_offset = usize::try_from(private_values.get(1).copied()? as i64).ok()?;
    let private_end = private_offset.checked_add(private_size)?;
    let private = cff_dict(bytes.get(private_offset..private_end)?);
    let default_width = private
        .get(&20)
        .and_then(|values| values.first().copied())
        .unwrap_or(0.0);
    let nominal_width = private
        .get(&21)
        .and_then(|values| values.first().copied())
        .unwrap_or(0.0);
    let width = cff_charstring_width(charstring, nominal_width, default_width)?;
    let matrix = top_dict
        .get(&1207)
        .and_then(|values| values.first().copied())
        .unwrap_or(0.001);
    Some(width * matrix * 1000.0)
}

fn cff_glyph_for_byte(
    cff: &ttf_parser::cff::Table<'_>,
    encoding: &SimpleFontEncoding,
    byte: u8,
) -> Option<ttf_parser::GlyphId> {
    match encoding.glyph_name(byte) {
        Some(name) => cff.glyph_index_by_name(name),
        None => cff.glyph_index(byte),
    }
}

fn type3_charproc_width(stream: &Stream, limits: &SafetyLimits) -> Result<Option<f64>, PdfError> {
    let bytes = decode_font_stream(stream, limits)?;
    let tokens = bytes
        .split(|byte| matches!(*byte, b'\r' | b'\n'))
        .flat_map(|line| line.split(|byte| *byte == b'%').next())
        .flat_map(|line| line.split(u8::is_ascii_whitespace))
        .filter(|token| !token.is_empty())
        .take(7)
        .collect::<Vec<_>>();
    let width = tokens
        .first()
        .and_then(|token| std::str::from_utf8(token).ok())
        .and_then(|token| token.parse::<f64>().ok());
    Ok(match tokens.as_slice() {
        [_, _, b"d0", ..] | [_, _, _, _, _, _, b"d1", ..] => width,
        _ => None,
    })
}

fn cff_charstring_width(bytes: &[u8], nominal: f64, default: f64) -> Option<f64> {
    let mut operands = Vec::new();
    let mut position = 0usize;
    while position < bytes.len() {
        let byte = *bytes.get(position)?;
        if let Some((value, consumed)) = cff_number(bytes.get(position..)?) {
            operands.push(value);
            position = position.checked_add(consumed)?;
            continue;
        }
        let operator = if byte == 12 {
            let second = *bytes.get(position)?;
            1200 + u16::from(second)
        } else {
            u16::from(byte)
        };
        if operator == 14 {
            return Some(if operands.len() == 1 {
                nominal + operands[0]
            } else {
                default
            });
        }
        return Some(if operands.len() % 2 == 1 {
            nominal + operands[0]
        } else {
            default
        });
    }
    None
}

fn cff_fd_select(bytes: &[u8], offset: usize, glyph_id: u16) -> Option<u8> {
    let format = *bytes.get(offset)?;
    match format {
        0 => bytes
            .get(offset.checked_add(1 + usize::from(glyph_id))?)
            .copied(),
        3 => {
            let count = u16::from_be_bytes([*bytes.get(offset + 1)?, *bytes.get(offset + 2)?]);
            let mut position = offset + 3;
            let mut previous = 0_u16;
            for _ in 0..count {
                let first = u16::from_be_bytes([*bytes.get(position)?, *bytes.get(position + 1)?]);
                let fd = *bytes.get(position + 2)?;
                if previous <= glyph_id && glyph_id < first {
                    return Some(fd);
                }
                previous = first;
                position += 3;
            }
            None
        }
        _ => None,
    }
}

fn cff_dict_offset(dict: &BTreeMap<u16, Vec<f64>>, operator: u16) -> Option<usize> {
    usize::try_from(dict.get(&operator)?.first().copied()? as i64).ok()
}

fn cff_dict(bytes: &[u8]) -> BTreeMap<u16, Vec<f64>> {
    let mut result = BTreeMap::new();
    let mut operands = Vec::new();
    let mut position = 0usize;
    while position < bytes.len() {
        if let Some((value, consumed)) = cff_number(&bytes[position..]) {
            operands.push(value);
            position += consumed;
            continue;
        }
        let byte = bytes[position];
        position += 1;
        let operator = if byte == 12 {
            let Some(second) = bytes.get(position).copied() else {
                break;
            };
            position += 1;
            1200 + u16::from(second)
        } else {
            u16::from(byte)
        };
        result.insert(operator, std::mem::take(&mut operands));
    }
    result
}

fn cff_number(bytes: &[u8]) -> Option<(f64, usize)> {
    let first = *bytes.first()?;
    match first {
        32..=246 => Some((f64::from(first) - 139.0, 1)),
        247..=250 => Some((
            f64::from(first - 247) * 256.0 + f64::from(*bytes.get(1)?) + 108.0,
            2,
        )),
        251..=254 => Some((
            -(f64::from(first - 251) * 256.0 + f64::from(*bytes.get(1)?) + 108.0),
            2,
        )),
        28 => Some((
            f64::from(i16::from_be_bytes([*bytes.get(1)?, *bytes.get(2)?])),
            3,
        )),
        29 => Some((
            f64::from(i32::from_be_bytes([
                *bytes.get(1)?,
                *bytes.get(2)?,
                *bytes.get(3)?,
                *bytes.get(4)?,
            ])),
            5,
        )),
        255 => Some((
            f64::from(i32::from_be_bytes([
                *bytes.get(1)?,
                *bytes.get(2)?,
                *bytes.get(3)?,
                *bytes.get(4)?,
            ])) / 65_536.0,
            5,
        )),
        _ => None,
    }
}

fn cff_index(bytes: &[u8], offset: usize) -> Option<(Vec<&[u8]>, usize)> {
    let count = usize::from(u16::from_be_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]));
    if count == 0 {
        return Some((Vec::new(), offset + 2));
    }
    let offsize = usize::from(*bytes.get(offset + 2)?);
    if !(1..=4).contains(&offsize) {
        return None;
    }
    let offsets_start = offset + 3;
    let data_start = offsets_start.checked_add((count + 1).checked_mul(offsize)?)?;
    let read_offset = |index: usize| -> Option<usize> {
        let start = offsets_start.checked_add(index.checked_mul(offsize)?)?;
        let mut value = 0usize;
        for byte in bytes.get(start..start + offsize)? {
            value = value.checked_mul(256)?.checked_add(usize::from(*byte))?;
        }
        (value > 0).then_some(value - 1)
    };
    let mut items = Vec::with_capacity(count);
    for index in 0..count {
        let start = data_start.checked_add(read_offset(index)?)?;
        let end = data_start.checked_add(read_offset(index + 1)?)?;
        items.push(bytes.get(start..end)?);
    }
    let end = data_start.checked_add(read_offset(count)?)?;
    Some((items, end))
}

fn type1_pfb_payload(bytes: &[u8]) -> Vec<u8> {
    if !bytes.starts_with(&[0x80, 0x01]) {
        return bytes.to_vec();
    }
    let mut payload = Vec::new();
    let mut position = 0;
    while bytes.get(position) == Some(&0x80) {
        let Some(kind) = bytes.get(position + 1) else {
            break;
        };
        if *kind == 3 {
            break;
        }
        let Some(length) = bytes
            .get(position + 2..position + 6)
            .and_then(|length| length.try_into().ok())
            .map(u32::from_le_bytes)
            .and_then(|length| usize::try_from(length).ok())
        else {
            break;
        };
        let start = position + 6;
        let Some(end) = start.checked_add(length) else {
            break;
        };
        let Some(segment) = bytes.get(start..end) else {
            break;
        };
        payload.extend_from_slice(segment);
        position = end;
    }
    payload
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
) -> Result<(Option<Vec<u8>>, bool, bool), PdfError> {
    let Ok(encoding) = font.get(b"Encoding") else {
        return Ok((None, false, true));
    };
    let Some(encoding) = resolve_optional(document, encoding, limits.max_reference_depth)? else {
        return Ok((None, false, true));
    };
    if let Ok(name) = encoding.as_name() {
        return Ok((Some(name.to_vec()), false, true));
    }
    let Ok(dictionary) = encoding.as_dict() else {
        return Ok((None, false, true));
    };
    let base = resolved_name(document, dictionary, b"BaseEncoding", limits)?.map(ToOwned::to_owned);
    let contains_differences = contains_key(dictionary, b"Differences");
    let differences_unicode_compliant = match dictionary.get(b"Differences") {
        Err(_) | Ok(Object::Null) => true,
        Ok(_) => dictionary.get_font_encoding(document).is_ok(),
    };
    Ok((base, contains_differences, differences_unicode_compliant))
}

/// The subset of an SFNT needed by veraPDF's glyph-presence and width model.
/// It deliberately does not parse `maxp`, `loca`, or `glyf`: the pinned
/// malformed-`maxp` fixture proves those unrelated tables do not make these
/// model properties inapplicable. The `hmtx` byte extent itself supplies a
/// bounded glyph-count upper bound.
struct RawTrueType<'a> {
    cmap: Option<ttf_parser::cmap::Table<'a>>,
    hmtx: Option<&'a [u8]>,
    number_of_h_metrics: Option<usize>,
    glyph_count: Option<usize>,
    units_per_em: Option<u16>,
}

impl<'a> RawTrueType<'a> {
    fn parse(bytes: &'a [u8]) -> Option<Self> {
        let face = ttf_parser::RawFace::parse(bytes, 0).ok()?;
        let head = face.table(ttf_parser::Tag::from_bytes(b"head"));
        let hhea = face.table(ttf_parser::Tag::from_bytes(b"hhea"));
        let hmtx = face.table(ttf_parser::Tag::from_bytes(b"hmtx"));
        let units_per_em = head
            .and_then(|head| be_u16(head, 18))
            .filter(|value| *value != 0);
        let number_of_h_metrics = hhea
            .and_then(|hhea| be_u16(hhea, 34))
            .map(usize::from)
            .filter(|value| *value != 0);
        let metric_glyph_count = number_of_h_metrics.and_then(|count| {
            let hmtx = hmtx?;
            let long_metrics_bytes = count.checked_mul(4)?;
            let bearings_bytes = hmtx.len().checked_sub(long_metrics_bytes)?;
            (bearings_bytes % 2 == 0).then(|| count + bearings_bytes / 2)
        });
        let maxp_glyph_count = face
            .table(ttf_parser::Tag::from_bytes(b"maxp"))
            .and_then(|maxp| be_u16(maxp, 4))
            .map(usize::from);
        let loca_glyph_count = head.and_then(|head| {
            let entry_size = match be_i16(head, 50)? {
                0 => 2,
                1 => 4,
                _ => return None,
            };
            let loca = face.table(ttf_parser::Tag::from_bytes(b"loca"))?;
            (loca.len() % entry_size == 0)
                .then(|| loca.len() / entry_size)
                .and_then(|entries| entries.checked_sub(1))
        });
        let glyph_count = maxp_glyph_count.or(loca_glyph_count).or(metric_glyph_count);
        let cmap = face
            .table(ttf_parser::Tag::from_bytes(b"cmap"))
            .and_then(ttf_parser::cmap::Table::parse);
        Some(Self {
            cmap,
            hmtx,
            number_of_h_metrics,
            glyph_count,
            units_per_em,
        })
    }

    fn glyph_index(&self, character: char) -> Option<ttf_parser::GlyphId> {
        for subtable in self.cmap?.subtables {
            if subtable.is_unicode()
                && let Some(glyph) = subtable.glyph_index(u32::from(character))
            {
                return Some(glyph);
            }
        }
        None
    }

    fn glyph_index_for_symbolic_byte(&self, byte: u8) -> Option<ttf_parser::GlyphId> {
        let cmap = self.cmap?;
        for subtable in cmap.subtables {
            let glyph = if subtable.is_unicode() {
                subtable.glyph_index(u32::from(byte))
            } else if subtable.platform_id == ttf_parser::PlatformId::Windows
                && subtable.encoding_id == 0
            {
                subtable
                    .glyph_index(0xF000 + u32::from(byte))
                    .or_else(|| subtable.glyph_index(u32::from(byte)))
            } else {
                subtable.glyph_index(u32::from(byte))
            };
            if glyph.is_some() {
                return glyph;
            }
        }
        None
    }

    fn glyph_hor_advance(&self, glyph: ttf_parser::GlyphId) -> Option<u16> {
        let glyph = usize::from(glyph.0);
        if glyph >= self.glyph_count? {
            return None;
        }
        let metric = glyph.min(self.number_of_h_metrics?.checked_sub(1)?);
        be_u16(self.hmtx?, metric.checked_mul(4)?)
    }

    fn glyph_is_present(&self, glyph: ttf_parser::GlyphId) -> bool {
        self.glyph_count
            .is_some_and(|count| usize::from(glyph.0) < count)
    }
}

fn be_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset.checked_add(1)?)?,
    ]))
}

fn be_i16(bytes: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_be_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset.checked_add(1)?)?,
    ]))
}

fn truetype_cmap_summary(
    document: &Document,
    descriptor: Option<&Dictionary>,
    limits: &SafetyLimits,
) -> Result<Option<(usize, bool)>, PdfError> {
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
    // Confirmed live against veraPDF 1.30.2: it reads the `cmap` table's
    // subtable count directly from the SFNT table directory, independent
    // of whether the rest of the font (`maxp`, `hhea`, ...) otherwise
    // parses -- a font whose `cmap` table is valid but whose `maxp` table
    // is malformed still gets this predicate evaluated. `RawFace` reads
    // only the table directory, unlike `Face::parse`, which additionally
    // requires several unrelated mandatory tables to succeed.
    Ok(ttf_parser::RawFace::parse(&bytes, 0)
        .ok()
        .and_then(|face| face.table(ttf_parser::Tag::from_bytes(b"cmap")))
        .and_then(ttf_parser::cmap::Table::parse)
        .map(|cmap| {
            let cmap_count = usize::from(cmap.subtables.len());
            let cmap30_present = cmap.subtables.into_iter().any(|subtable| {
                subtable.platform_id == ttf_parser::PlatformId::Windows && subtable.encoding_id == 0
            });
            (cmap_count, cmap30_present)
        }))
}

fn font_is_embedded(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Some(descriptor) = font_descriptor_dictionary(document, font, limits)? else {
        return Ok(false);
    };
    // `containsFontFile` is a veraPDF model property, not mere key presence:
    // the pinned malformed-program case proves that the stream must be
    // recognized as an embedded font program.
    for key in [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"] {
        if let Ok(file) = descriptor.get(key)
            && let Some(stream) = resolve_optional(document, file, limits.max_reference_depth)?
                .and_then(|object| object.as_stream().ok())
            && valid_font_program(document, key, stream, limits)?
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
) -> Result<Option<(String, bool)>, PdfError> {
    let Some(descriptor) = font_descriptor_dictionary(document, font, limits)? else {
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
        let Some(subtype) = resolved_name(document, &stream.dict, b"Subtype", limits)? else {
            continue;
        };
        if !matches!(subtype, b"Type1C" | b"CIDFontType0C") {
            let valid_pdfa2 = key == b"FontFile3" && subtype == b"OpenType";
            return Ok(Some((
                String::from_utf8_lossy(subtype).into_owned(),
                valid_pdfa2,
            )));
        }
    }
    Ok(None)
}

fn valid_font_program(
    document: &Document,
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
        // Confirmed live against veraPDF 1.30.2: a stream whose bytes start
        // with the Type1 magic header but never contain an `eexec` marker
        // (no encrypted/binary section at all, just header text) still
        // fails `PDFA1B-FONT-EMBEDDING-001` on veraPDF -- the header alone
        // is not sufficient, matching the PFB/Type1 format's own
        // requirement that a real program always has an encrypted segment.
        b"FontFile" => {
            (bytes.starts_with(b"%!PS-AdobeFont")
                && bytes.windows(5).any(|window| window == b"eexec"))
                || (bytes.starts_with(&[0x80, 0x01])
                    && bytes.windows(2).any(|window| window == [0x80, 0x02]))
        }
        b"FontFile2" => valid_sfnt(&bytes),
        // Confirmed live against veraPDF 1.30.2: a stream whose bytes
        // satisfy only the CFF header-byte shape (major/minor version,
        // hdrSize) but contain no parseable CFF structure beyond that still
        // fails `PDFA1B-FONT-EMBEDDING-001` -- the header alone is not
        // sufficient. Uses the same `ttf_parser::cff::Table::parse` already
        // relied on for glyph lookups (`inspect_rendered_cff_type1_glyphs`/
        // `inspect_rendered_cff_cidfont_glyphs`), for consistency.
        b"FontFile3" => {
            matches!(
                resolved_name(document, &stream.dict, b"Subtype", limits)?,
                Some(b"Type1C" | b"CIDFontType0C")
            ) && ttf_parser::cff::Table::parse(&bytes).is_some()
                || resolved_name(document, &stream.dict, b"Subtype", limits)?
                    == Some(b"OpenType".as_slice())
                    && valid_sfnt(&bytes)
        }
        _ => false,
    })
}

// Confirmed live against veraPDF 1.30.2 (the same fixture that confirmed
// `truetype_cmap_count`'s fix): it still considers a `/FontFile2` stream
// "embedded" (no `PDFA1B-FONT-EMBEDDING-001` failure) even when the font's
// `maxp` table is malformed enough that a full `ttf_parser::Face::parse`
// fails, as long as the SFNT signature and table directory are themselves
// readable. `RawFace::parse` reads only the table directory, matching that
// narrower bar, instead of requiring every mandatory table
// (`head`/`hhea`/`maxp`/`hmtx`/glyph outlines) to individually succeed.
fn valid_sfnt(bytes: &[u8]) -> bool {
    ttf_parser::RawFace::parse(bytes, 0).is_ok()
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

fn describe_descendant(object: &Object, parent: &str) -> String {
    match object {
        Object::Reference((number, generation)) => {
            format!("descendant font object {number} {generation}")
        }
        _ => format!("direct descendant font of {parent}"),
    }
}

pub(crate) fn shown_text_bytes(operands: &[Object]) -> Vec<u8> {
    let mut bytes = Vec::new();
    collect_shown_text_bytes(operands, &mut bytes);
    bytes
}

pub(crate) fn type3_glyph_names(
    document: &Document,
    font: &Dictionary,
    shown_bytes: &[u8],
    limits: &SafetyLimits,
) -> Result<Vec<String>, PdfError> {
    let encoding = simple_font_encoding(document, font, limits)?;
    Ok(shown_bytes
        .iter()
        .copied()
        .filter_map(|byte| encoding.glyph_name(byte).map(ToOwned::to_owned))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

fn collect_shown_text_bytes(operands: &[Object], bytes: &mut Vec<u8>) {
    for operand in operands {
        match operand {
            Object::String(value, _) => bytes.extend_from_slice(value),
            Object::Array(items) => collect_shown_text_bytes(items, bytes),
            _ => {}
        }
    }
}

/// Resolves `object` through indirection before checking whether it is an
/// integer. Confirmed live against veraPDF 1.30.2 that an *indirect*
/// numeric value (e.g. a CMap's `/WMode`) is resolved rather than treated
/// as absent -- the several integer-valued keys this file reads (`/WMode`,
/// `/Flags`, `/FirstChar`, `/LastChar`) are not guaranteed to be direct.
fn resolved_integer(
    document: &Document,
    limits: &SafetyLimits,
    object: &Object,
) -> Result<Option<i64>, PdfError> {
    Ok(
        resolve_optional(document, object, limits.max_reference_depth)?
            .and_then(|object| object.as_i64().ok()),
    )
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
    use lopdf::{Dictionary, Document, Object, Stream};

    use super::{
        CidSystemInfo, UnicodeCmap, cff_fd_select, cff_index, cmap_bytes_system_info,
        cmap_maximal_cid, cmap_uses_identity_base, inspect_all_embedded_cmap_cids, parse_cmap,
        parse_cmap_with_predefined_bases, shown_text_bytes, type1_eexec_ciphertext,
        type1_pfb_payload, type1_program_char_names, type1_program_charstring_widths,
    };
    use crate::{SafetyLimits, predefined_cmaps};

    /// A malicious CMap can declare an astronomical entry count while
    /// supplying almost no real tokens (`99999999999999 begincidrange <00>
    /// <01> 1 endcidrange` is ~50 bytes). Before `bounded_cmap_entry_count`
    /// existed, both `cmap_maximal_cid` and `parse_cmap` trusted the
    /// declared count directly as a loop bound, so this alone hung the
    /// validator (confirmed: still running after 30+ seconds). The fix
    /// clamps the count to what the token buffer could actually hold, so
    /// this must now return almost immediately.
    #[test]
    fn huge_declared_cmap_entry_count_does_not_hang() {
        let malicious = b"99999999999999 begincidrange <00> <01> 1 endcidrange";
        let started = std::time::Instant::now();
        assert_eq!(cmap_maximal_cid(malicious), Some(2));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "cmap_maximal_cid took {:?} on a malicious declared count",
            started.elapsed()
        );

        let malicious_parse =
            b"1 begincodespacerange <00> <FF> endcodespacerange 99999999999999 begincidrange <00> <01> 1 endcidrange";
        let started = std::time::Instant::now();
        let parsed = parse_cmap(malicious_parse).expect("parse malicious cmap");
        assert_eq!(parsed.decode(&[0x00]), Some(vec![1]));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "parse_cmap took {:?} on a malicious declared count",
            started.elapsed()
        );
    }

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
        assert_eq!(
            cmap_maximal_cid(
                b"1 beginnotdefchar <00> 65536 endnotdefchar 1 beginnotdefrange <01> <ff> 12 endnotdefrange"
            ),
            Some(65_536)
        );
    }

    #[test]
    fn decodes_explicit_variable_width_cid_cmaps() {
        let cmap = b"% 1 begincidchar <41> 99 endcidchar\n2 begincodespacerange\n<00> <7F>\n<8000> <80FF>\nendcodespacerange\n1 begincidchar\n<41> 12\nendcidchar\n1 begincidrange\n<8000> <8002> 20\nendcidrange";
        let cmap_cids =
            |shown_bytes: &[u8]| parse_cmap(cmap).and_then(|parsed| parsed.decode(shown_bytes));
        assert_eq!(cmap_cids(&[0x41, 0x80, 0x01]), Some(vec![12, 21]));
        assert_eq!(cmap_cids(&[0x80]), None);
        assert_eq!(cmap_cids(&[0x42]), None);
    }

    #[test]
    fn parses_usable_unicode_cmaps_and_rejects_incomplete_or_invalid_values() {
        let map = UnicodeCmap::parse(
            b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <41> <0041> endbfchar",
        )
        .expect("valid ToUnicode CMap");
        assert!(map.maps_usable(b"A"));
        assert!(!map.maps_usable(b"B"));
        assert!(UnicodeCmap::parse(b"1 beginbfchar <41> <d800> endbfchar").is_none());
        assert!(UnicodeCmap::parse(b"1 beginbfrange <00> <ffff> <d800> endbfrange").is_none());
    }

    #[test]
    fn decodes_notdef_cid_mappings_only_after_explicit_mappings() {
        let cmap = b"1 begincodespacerange <00> <ff> endcodespacerange 1 begincidchar <41> 7 endcidchar 1 beginnotdefrange <00> <ff> 1 endnotdefrange";
        let parsed = parse_cmap(cmap).expect("parse notdef CMap");
        assert_eq!(parsed.decode(b"A"), Some(vec![7]));
        assert_eq!(parsed.decode(b"B"), Some(vec![1]));
    }

    #[test]
    fn recognizes_identity_usecmap_bases() {
        assert!(cmap_uses_identity_base(b"/Identity-H usecmap"));
        assert!(cmap_uses_identity_base(b"/Identity-V\nusecmap"));
        assert!(!cmap_uses_identity_base(b"/NotIdentity usecmap"));
    }

    #[test]
    fn decodes_the_pinned_named_predefined_cmap_collection() {
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/predefined_cmaps");
        let mut checked = 0;
        for entry in std::fs::read_dir(directory).expect("read predefined CMaps") {
            let entry = entry.expect("predefined CMap entry");
            let name = entry
                .file_name()
                .into_string()
                .expect("ASCII predefined CMap name");
            let bytes = std::fs::read(entry.path()).expect("read predefined CMap");
            assert_eq!(
                predefined_cmaps::get(name.as_bytes()),
                Some(bytes.as_slice())
            );
            if cmap_tokens_have_cid_mapping_or_base(&bytes) {
                assert!(
                    parse_cmap_with_predefined_bases(&bytes).is_some(),
                    "could not parse predefined CMap {name}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 113);
    }

    fn cmap_tokens_have_cid_mapping_or_base(bytes: &[u8]) -> bool {
        super::cmap_tokens(bytes)
            .iter()
            .any(|token| matches!(*token, b"begincidchar" | b"begincidrange" | b"usecmap"))
    }

    #[test]
    fn named_predefined_cmaps_supply_cids_and_system_info() {
        let horizontal = predefined_cmaps::get(b"UniJIS-UCS2-H").expect("UniJIS CMap");
        let decoder = parse_cmap_with_predefined_bases(horizontal).expect("parse UniJIS CMap");
        assert_eq!(decoder.decode(&[0x00, 0x3f]), Some(vec![32]));
        assert_eq!(
            cmap_bytes_system_info(horizontal),
            Some(CidSystemInfo {
                registry: b"Adobe".to_vec(),
                ordering: b"Japan1".to_vec(),
                supplement: Some(4),
            })
        );

        let vertical = predefined_cmaps::get(b"UniJIS-UCS2-V").expect("vertical UniJIS CMap");
        let decoder = parse_cmap_with_predefined_bases(vertical).expect("parse vertical UniJIS");
        assert_eq!(decoder.decode(&[0x00, 0x3f]), Some(vec![32]));
    }

    #[test]
    fn extracts_names_from_an_encrypted_type1_private_dictionary() {
        let plaintext = [
            vec![0; 4],
            b"dup /Private 1 dict dup begin /CharStrings 1 dict dup begin /space 1 RD".to_vec(),
        ]
        .concat();
        let mut state = 55_665_u16;
        let encrypted: Vec<_> = plaintext
            .into_iter()
            .map(|plaintext| {
                let ciphertext = plaintext ^ (state >> 8) as u8;
                state = state
                    .wrapping_add(u16::from(ciphertext))
                    .wrapping_mul(52_845)
                    .wrapping_add(22_719);
                ciphertext
            })
            .collect();
        let bytes = [b"%!PS-AdobeFont\neexec\n".as_slice(), encrypted.as_slice()].concat();
        assert!(super::type1_program_char_names(&bytes).contains("space"));

        let hex = encrypted
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        let hex_bytes = [b"%!PS-AdobeFont\neexec\n".as_slice(), hex.as_bytes()].concat();
        assert!(super::type1_program_char_names(&hex_bytes).contains("space"));
    }

    #[test]
    fn maps_printable_type1_encoding_bytes_to_adobe_glyph_names() {
        assert_eq!(
            crate::font_encodings::glyph_name(
                crate::font_encodings::PredefinedEncoding::Standard,
                b' '
            ),
            Some("space")
        );
        assert_eq!(
            crate::font_encodings::glyph_name(
                crate::font_encodings::PredefinedEncoding::Standard,
                174
            ),
            Some("fi")
        );
    }

    #[test]
    fn parses_custom_and_standard_based_type1_program_encodings() {
        let custom = super::type1_program_encoding(
            b"%!PS-AdobeFont-1.0\n/Encoding 256 array dup 65 /Alpha put readonly def\neexec\n",
        );
        assert_eq!(custom.glyph_name(65), Some("Alpha"));
        assert_eq!(custom.glyph_name(66), None);

        let based = super::type1_program_encoding(
            b"%!PS-AdobeFont-1.0\n/Encoding StandardEncoding 256 array copy dup 65 /Alpha put readonly def\neexec\n",
        );
        assert_eq!(based.glyph_name(65), Some("Alpha"));
        assert_eq!(based.glyph_name(66), Some("B"));
    }

    #[test]
    fn parses_type3_d0_and_d1_widths() {
        let limits = SafetyLimits::default();
        for (bytes, expected) in [
            (b"500 0 d0\n".as_slice(), 500.0),
            (b"400 0 0 0 500 700 d1\n".as_slice(), 400.0),
        ] {
            let stream = Stream::new(Dictionary::new(), bytes.to_vec());
            assert_eq!(
                super::type3_charproc_width(&stream, &limits).expect("parse Type3 CharProc"),
                Some(expected)
            );
        }
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

    /// Every hand-rolled font byte-parser in this file must not panic on
    /// truncated or empty input, since embedded font programs are attacker
    /// controlled. Reaching the final assertions at all (rather than
    /// aborting on a panic) is the test; the specific `None`/empty return
    /// values are incidental. This does not exercise `ttf_parser`/`lopdf`
    /// -backed paths (`RawFace`, `cff::Table`), which are already
    /// panic-free by the external crate's own contract.
    #[test]
    fn hand_rolled_font_parsers_do_not_panic_on_truncated_input() {
        for truncated in [b"".as_slice(), b"\x00", b"\x00\x01\x02", b"\x80\x01\x00"] {
            let _ = cff_fd_select(truncated, 0, 0);
            let _ = cff_index(truncated, 0);
            let _ = parse_cmap(truncated);
            assert!(type1_program_char_names(truncated).is_empty());
            assert!(type1_program_charstring_widths(truncated).is_empty());
            let _ = type1_eexec_ciphertext(truncated);
            let _ = type1_pfb_payload(truncated);
        }
        // A `cff_fd_select` format-3 range table declaring a huge count
        // with almost no backing bytes must not hang (same class of bug as
        // `bounded_cmap_entry_count` guards for CMaps): each iteration's
        // `bytes.get(...)` bounds check fails immediately once the real
        // data is exhausted, since `count` is read as a fixed 2-byte `u16`
        // (capped at 65,535) rather than parsed from unbounded decimal
        // text.
        let huge_count_fd_select = [3u8, 0xFF, 0xFF, 0, 1, 0];
        let started = std::time::Instant::now();
        assert_eq!(cff_fd_select(&huge_count_fd_select, 0, 5), None);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn retains_only_string_bytes_from_text_show_operands() {
        assert_eq!(
            shown_text_bytes(&[
                Object::String(b"A".to_vec(), lopdf::StringFormat::Literal),
                Object::Array(vec![
                    Object::String(b"B".to_vec(), lopdf::StringFormat::Literal),
                    Object::Integer(-120),
                    Object::String(b"C".to_vec(), lopdf::StringFormat::Literal),
                ]),
            ]),
            b"ABC"
        );
    }
}
