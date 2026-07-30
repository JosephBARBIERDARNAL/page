use std::collections::{BTreeMap, BTreeSet};

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::content_support::{
    ContentCache, decode_content_stream_cached, inherited_page_resources, resource_once,
};
use crate::error::PdfError;
use crate::graphics::is_standard_rendering_intent;
use crate::limits::SafetyLimits;
use crate::model::IccHeader;
use crate::object_resolution::{ResourceKey, resolve_optional};
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct IccBasedSummary {
    pub(crate) failures: Vec<RuleFailure>,
    pub(crate) component_failures: Vec<RuleFailure>,
    pub(crate) device_gray_context: Option<String>,
    pub(crate) device_rgb_context: Option<String>,
    pub(crate) device_cmyk_context: Option<String>,
    pub(crate) used_xobject_ids: BTreeSet<ObjectId>,
    pub(crate) used_extgstate_ids: BTreeSet<ObjectId>,
    pub(crate) used_direct_extgstates: Vec<Dictionary>,
    pub(crate) invalid_rendering_intents: BTreeMap<String, String>,
    pub(crate) undefined_operators: BTreeMap<String, String>,
    pub(crate) inline_image_lzw_context: Option<String>,
    pub(crate) invalid_devicen_components: Vec<RuleFailure>,
}

#[derive(Clone, Copy, Debug)]
enum DeviceColorSpace {
    Gray,
    Rgb,
    Cmyk,
}

struct Scanner<'a> {
    document: &'a Document,
    limits: &'a SafetyLimits,
    cache: &'a mut ContentCache,
    inspected_profiles: BTreeSet<ResourceKey>,
    failures: BTreeMap<ResourceKey, RuleFailure>,
    component_failures: BTreeMap<ResourceKey, RuleFailure>,
    device_gray_context: Option<String>,
    device_rgb_context: Option<String>,
    device_cmyk_context: Option<String>,
    used_xobject_ids: BTreeSet<ObjectId>,
    used_extgstate_ids: BTreeSet<ObjectId>,
    used_direct_extgstates: Vec<Dictionary>,
    invalid_rendering_intents: BTreeMap<String, String>,
    undefined_operators: BTreeMap<String, String>,
    inline_image_lzw_context: Option<String>,
    invalid_devicen_components: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    pages: &BTreeMap<u32, ObjectId>,
    cache: &mut ContentCache,
    limits: &SafetyLimits,
) -> Result<IccBasedSummary, PdfError> {
    let mut scanner = Scanner {
        document,
        limits,
        cache,
        inspected_profiles: BTreeSet::new(),
        failures: BTreeMap::new(),
        component_failures: BTreeMap::new(),
        device_gray_context: None,
        device_rgb_context: None,
        device_cmyk_context: None,
        used_xobject_ids: BTreeSet::new(),
        used_extgstate_ids: BTreeSet::new(),
        used_direct_extgstates: Vec::new(),
        invalid_rendering_intents: BTreeMap::new(),
        undefined_operators: BTreeMap::new(),
        inline_image_lzw_context: None,
        invalid_devicen_components: Vec::new(),
    };
    for (&page_number, &page_id) in pages {
        scanner.scan_page(page_number, page_id)?;
    }
    scanner
        .invalid_devicen_components
        .extend(inspect_all_devicen_components(document, limits)?);
    scanner.invalid_devicen_components.sort_by(|left, right| {
        left.object_id
            .cmp(&right.object_id)
            .then_with(|| left.description.cmp(&right.description))
    });
    scanner.invalid_devicen_components.dedup_by(|left, right| {
        left.object_id == right.object_id && left.description == right.description
    });
    Ok(IccBasedSummary {
        failures: scanner.failures.into_values().collect(),
        component_failures: scanner.component_failures.into_values().collect(),
        device_gray_context: scanner.device_gray_context,
        device_rgb_context: scanner.device_rgb_context,
        device_cmyk_context: scanner.device_cmyk_context,
        used_xobject_ids: scanner.used_xobject_ids,
        used_extgstate_ids: scanner.used_extgstate_ids,
        used_direct_extgstates: scanner.used_direct_extgstates,
        invalid_rendering_intents: scanner.invalid_rendering_intents,
        undefined_operators: scanner.undefined_operators,
        inline_image_lzw_context: scanner.inline_image_lzw_context,
        invalid_devicen_components: scanner.invalid_devicen_components,
    })
}

fn inspect_all_devicen_components(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<Vec<RuleFailure>, PdfError> {
    let mut failures = Vec::new();
    let mut visited = BTreeSet::new();
    for (object_id, object) in &document.objects {
        inspect_devicen_object(
            document,
            object,
            Some((*object_id).into()),
            limits,
            &mut visited,
            &mut failures,
        )?;
    }
    Ok(failures)
}

fn devicen_component_count(
    document: &Document,
    items: &[Object],
    limits: &SafetyLimits,
) -> Result<usize, PdfError> {
    Ok(items
        .get(1)
        .map(|components| resolve_optional(document, components, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|components| components.as_array().ok())
        .map_or(0, Vec::len))
}

fn inspect_devicen_object(
    document: &Document,
    object: &Object,
    owner: Option<crate::PdfObjectId>,
    limits: &SafetyLimits,
    visited: &mut BTreeSet<ObjectId>,
    failures: &mut Vec<RuleFailure>,
) -> Result<(), PdfError> {
    match object {
        Object::Reference(object_id) if visited.insert(*object_id) => {
            if let Ok(referenced) = document.get_object(*object_id) {
                inspect_devicen_object(
                    document,
                    referenced,
                    Some((*object_id).into()),
                    limits,
                    visited,
                    failures,
                )?;
            }
        }
        Object::Array(items) => {
            if items.first().and_then(|item| item.as_name().ok()) == Some(b"DeviceN".as_slice()) {
                let components = devicen_component_count(document, items, limits)?;
                if components > 8 {
                    failures.push(RuleFailure {
                        object_id: owner,
                        description: format!(
                            "an object uses a DeviceN colour space with {components} components"
                        ),
                    });
                }
            }
            for item in items {
                inspect_devicen_object(document, item, owner, limits, visited, failures)?;
            }
        }
        Object::Dictionary(dictionary) => {
            for (_, value) in dictionary.iter() {
                inspect_devicen_object(document, value, owner, limits, visited, failures)?;
            }
        }
        Object::Stream(stream) => {
            for (_, value) in stream.dict.iter() {
                inspect_devicen_object(document, value, owner, limits, visited, failures)?;
            }
        }
        _ => {}
    }
    Ok(())
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
        let mut active_xobjects = BTreeSet::new();
        let mut decoded_bytes = 0usize;
        if let Ok(contents) = page.get(b"Contents") {
            self.scan_contents(
                contents,
                resources,
                resources,
                &mut active_xobjects,
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
        page_resources: Option<&Dictionary>,
        active_xobjects: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let content_id = contents.as_reference().ok();
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
                    page_resources,
                    active_xobjects,
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
        let bytes = decode_content_stream_cached(
            stream,
            content_id,
            self.cache,
            self.limits,
            decoded_bytes,
        )?;
        for name in inline_image_color_space_names(&bytes) {
            self.inspect_selected_color_space(
                &Object::Name(name),
                resources,
                page_resources,
                &format!("{context}/inline image"),
            )?;
        }
        if inline_image_has_lzw_filter(&bytes) {
            self.inline_image_lzw_context
                .get_or_insert_with(|| format!("{context}/inline image"));
        }
        let content_bytes = content_without_inline_images(&bytes);
        let Ok(content) = Content::decode(&content_bytes) else {
            return Ok(());
        };
        for operation in content.operations {
            if !is_pdf_1_4_operator(&operation.operator) {
                self.undefined_operators
                    .entry(operation.operator.clone())
                    .or_insert_with(|| context.to_owned());
            }
            match operation.operator.as_str() {
                "CS" | "cs" => {
                    if let Some(color_space) = operation.operands.first() {
                        self.inspect_selected_color_space(
                            color_space,
                            resources,
                            page_resources,
                            &format!("{context}/{}", operation.operator),
                        )?;
                    }
                }
                "g" | "G" => {
                    self.inspect_default_color_space(
                        b"DefaultGray",
                        DeviceColorSpace::Gray,
                        resources,
                        page_resources,
                        context,
                    )?;
                }
                "rg" | "RG" => {
                    self.inspect_default_color_space(
                        b"DefaultRGB",
                        DeviceColorSpace::Rgb,
                        resources,
                        page_resources,
                        context,
                    )?;
                }
                "k" | "K" => {
                    self.inspect_default_color_space(
                        b"DefaultCMYK",
                        DeviceColorSpace::Cmyk,
                        resources,
                        page_resources,
                        context,
                    )?;
                }
                "ri" => {
                    if let Some(name) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok())
                    {
                        let name = String::from_utf8_lossy(name).into_owned();
                        if !is_standard_rendering_intent(&name) {
                            self.invalid_rendering_intents
                                .entry(name)
                                .or_insert_with(|| context.to_owned());
                        }
                    }
                }
                "gs" => {
                    let Some(name) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok())
                    else {
                        continue;
                    };
                    match resource(
                        self.document,
                        self.limits,
                        resources,
                        page_resources,
                        b"ExtGState",
                        name,
                    )? {
                        Some(Object::Reference(id)) => {
                            self.used_extgstate_ids.insert(*id);
                        }
                        Some(Object::Dictionary(dictionary)) => {
                            self.used_direct_extgstates.push(dictionary.clone());
                        }
                        _ => {}
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
                    let Some(xobject) = resource(
                        self.document,
                        self.limits,
                        resources,
                        page_resources,
                        b"XObject",
                        name,
                    )?
                    else {
                        continue;
                    };
                    self.inspect_xobject(
                        xobject,
                        resources,
                        page_resources,
                        active_xobjects,
                        decoded_bytes,
                        &format!("{context}/XObject /{}", String::from_utf8_lossy(name)),
                        depth + 1,
                    )?;
                }
                "sh" => {
                    let Some(name) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok())
                    else {
                        continue;
                    };
                    let Some(shading) = resource(
                        self.document,
                        self.limits,
                        resources,
                        page_resources,
                        b"Shading",
                        name,
                    )?
                    else {
                        continue;
                    };
                    let Some(shading) =
                        resolve_optional(self.document, shading, self.limits.max_reference_depth)?
                            .and_then(|object| object.as_dict().ok())
                    else {
                        continue;
                    };
                    if let Ok(color_space) = shading.get(b"ColorSpace") {
                        self.inspect_selected_color_space(
                            color_space,
                            resources,
                            page_resources,
                            &format!("{context}/Shading /{}", String::from_utf8_lossy(name)),
                        )?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn inspect_xobject(
        &mut self,
        object: &Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        active_xobjects: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        let object_id = object.as_reference().ok();
        if let Some(object_id) = object_id {
            self.used_xobject_ids.insert(object_id);
        }
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        if object_id.is_some_and(|id| !active_xobjects.insert(id)) {
            return Ok(());
        }
        let result = self.inspect_xobject_inner(
            object,
            resources,
            page_resources,
            active_xobjects,
            decoded_bytes,
            context,
            depth,
        );
        if let Some(id) = object_id {
            active_xobjects.remove(&id);
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn inspect_xobject_inner(
        &mut self,
        object: &Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        active_xobjects: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        let Some(stream) =
            resolve_optional(self.document, object, self.limits.max_reference_depth)?
                .and_then(|object| object.as_stream().ok())
        else {
            return Ok(());
        };
        match stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
        {
            Some(b"Image") => {
                let image_mask = stream
                    .dict
                    .get(b"ImageMask")
                    .ok()
                    .and_then(|value| value.as_bool().ok())
                    == Some(true);
                if !image_mask && let Ok(color_space) = stream.dict.get(b"ColorSpace") {
                    self.inspect_selected_color_space(
                        color_space,
                        resources,
                        page_resources,
                        context,
                    )?;
                }
                for key in [b"Mask".as_slice(), b"SMask"] {
                    if let Ok(id) = stream.dict.get(key).and_then(|value| value.as_reference()) {
                        self.used_xobject_ids.insert(id);
                    }
                }
                if let Ok(alternates) = stream.dict.get(b"Alternates")
                    && let Some(alternates) = resolve_optional(
                        self.document,
                        alternates,
                        self.limits.max_reference_depth,
                    )?
                    .and_then(|object| object.as_array().ok())
                {
                    for (index, alternate) in alternates.iter().enumerate() {
                        let Some(alternate) = resolve_optional(
                            self.document,
                            alternate,
                            self.limits.max_reference_depth,
                        )?
                        .and_then(|object| object.as_dict().ok()) else {
                            continue;
                        };
                        if let Ok(image) = alternate.get(b"Image") {
                            self.inspect_xobject(
                                image,
                                resources,
                                page_resources,
                                active_xobjects,
                                decoded_bytes,
                                &format!("{context}/Alternate {index}"),
                                depth + 1,
                            )?;
                        }
                    }
                }
            }
            Some(b"Form") => {
                let form_resources = match stream.dict.get(b"Resources") {
                    Ok(entry) => {
                        resolve_optional(self.document, entry, self.limits.max_reference_depth)?
                            .and_then(|object| object.as_dict().ok())
                    }
                    Err(_) => None,
                };
                self.scan_contents(
                    object,
                    form_resources,
                    page_resources,
                    active_xobjects,
                    decoded_bytes,
                    context,
                    depth,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn inspect_selected_color_space(
        &mut self,
        value: &Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        context: &str,
    ) -> Result<(), PdfError> {
        if let Ok(name) = value.as_name()
            && let Some(color_space) = resource(
                self.document,
                self.limits,
                resources,
                page_resources,
                b"ColorSpace",
                name,
            )?
        {
            return self.inspect_color_space(
                color_space,
                &format!("{context} /{}", String::from_utf8_lossy(name)),
            );
        }
        self.inspect_color_space(value, context)
    }

    fn inspect_default_color_space(
        &mut self,
        name: &[u8],
        fallback: DeviceColorSpace,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        context: &str,
    ) -> Result<(), PdfError> {
        if let Some(color_space) = resource(
            self.document,
            self.limits,
            resources,
            page_resources,
            b"ColorSpace",
            name,
        )? {
            self.inspect_color_space(
                color_space,
                &format!("{context}/{}", String::from_utf8_lossy(name)),
            )?;
        } else {
            self.record_device_color(fallback, context);
        }
        Ok(())
    }

    fn inspect_color_space(&mut self, value: &Object, context: &str) -> Result<(), PdfError> {
        self.inspect_device_color_space_at_depth(value, context, 0)?;
        self.inspect_icc_color_space_at_depth(value, context, 0)
    }

    fn inspect_device_color_space_at_depth(
        &mut self,
        value: &Object,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let Some(value) = resolve_optional(self.document, value, self.limits.max_reference_depth)?
        else {
            return Ok(());
        };
        if let Ok(name) = value.as_name() {
            let device = match name {
                b"DeviceGray" | b"G" => Some(DeviceColorSpace::Gray),
                b"DeviceRGB" | b"RGB" => Some(DeviceColorSpace::Rgb),
                b"DeviceCMYK" | b"CMYK" => Some(DeviceColorSpace::Cmyk),
                _ => None,
            };
            if let Some(device) = device {
                self.record_device_color(device, context);
            }
            return Ok(());
        }
        let Ok(items) = value.as_array() else {
            return Ok(());
        };
        let kind = items.first().and_then(|item| item.as_name().ok());
        if kind == Some(b"DeviceN".as_slice()) {
            let components = devicen_component_count(self.document, items, self.limits)?;
            if components > 8 {
                self.invalid_devicen_components.push(RuleFailure {
                    object_id: value.as_reference().ok().map(Into::into),
                    description: format!(
                        "{context} uses a DeviceN colour space with {components} components"
                    ),
                });
            }
        }
        let nested = match kind {
            Some(b"Indexed") => items.get(1),
            Some(b"Separation") | Some(b"DeviceN") => items.get(2),
            _ => None,
        };
        if let Some(nested) = nested {
            self.inspect_device_color_space_at_depth(
                nested,
                &format!("{context}/alternate or base"),
                depth + 1,
            )?;
        }
        Ok(())
    }

    fn inspect_icc_color_space_at_depth(
        &mut self,
        value: &Object,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let Some(value) = resolve_optional(self.document, value, self.limits.max_reference_depth)?
        else {
            return Ok(());
        };
        let Ok(items) = value.as_array() else {
            return Ok(());
        };
        match items.first().and_then(|item| item.as_name().ok()) {
            Some(b"Indexed") => {
                if let Some(base) = items.get(1) {
                    self.inspect_icc_color_space_at_depth(
                        base,
                        &format!("{context}/Indexed base"),
                        depth + 1,
                    )?;
                }
                return Ok(());
            }
            Some(b"ICCBased") => {}
            _ => return Ok(()),
        }
        let Some(profile) = items.get(1) else {
            return Ok(());
        };
        let key = match profile {
            Object::Reference(id) => ResourceKey::Indirect(*id),
            _ => ResourceKey::Direct(context.to_owned()),
        };
        if !self.inspected_profiles.insert(key.clone()) {
            return Ok(());
        }
        let Some(stream) =
            resolve_optional(self.document, profile, self.limits.max_reference_depth)?
                .and_then(|object| object.as_stream().ok())
        else {
            return Ok(());
        };
        let bytes =
            match stream.decompressed_content_with_limit(self.limits.max_decoded_stream_size) {
                Ok(bytes) => bytes,
                Err(
                    error @ lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded {
                        ..
                    }),
                ) => {
                    return Err(PdfError::IccDecodeLimit(format!(
                        "ICCBased profile: {error}"
                    )));
                }
                Err(error) => {
                    self.record_failure(
                        key.clone(),
                        format!("could not decode ICCBased profile stream: {error}"),
                    );
                    self.record_component_failure(
                        key,
                        "could not decode the ICCBased profile colour space".to_owned(),
                    );
                    return Ok(());
                }
            };
        let Some(header) = IccHeader::parse(&bytes) else {
            self.record_failure(
                key.clone(),
                "ICCBased profile is shorter than the 20-byte header prefix required by this check"
                    .to_owned(),
            );
            self.record_component_failure(
                key,
                "ICCBased profile is too short to determine its data colour space".to_owned(),
            );
            return Ok(());
        };
        if !header.conforms_to_pdfa_1_input_profile() {
            self.record_failure(
                key.clone(),
                format!(
                    "ICCBased profile has class {:?}, colour space {:?}, and version {}.{}",
                    header.device_class,
                    header.color_space,
                    header.version_major,
                    header.version_minor
                ),
            );
        }
        let components = stream
            .dict
            .get(b"N")
            .ok()
            .and_then(|value| value.as_i64().ok());
        let components_match = matches!(
            (components, header.color_space.as_str()),
            (Some(1), "GRAY") | (Some(3), "RGB " | "Lab ") | (Some(4), "CMYK")
        );
        if !components_match {
            self.record_component_failure(
                key,
                format!(
                    "ICCBased profile has /N {components:?} and data colour space {:?}",
                    header.color_space
                ),
            );
        }
        Ok(())
    }

    fn record_device_color(&mut self, device: DeviceColorSpace, context: &str) {
        let destination = match device {
            DeviceColorSpace::Gray => &mut self.device_gray_context,
            DeviceColorSpace::Rgb => &mut self.device_rgb_context,
            DeviceColorSpace::Cmyk => &mut self.device_cmyk_context,
        };
        if destination.is_none() {
            *destination = Some(context.to_owned());
        }
    }

    fn record_failure(&mut self, key: ResourceKey, description: String) {
        let object_id = key.object_id();
        self.failures.entry(key).or_insert(RuleFailure {
            object_id,
            description,
        });
    }

    fn record_component_failure(&mut self, key: ResourceKey, description: String) {
        let object_id = key.object_id();
        self.component_failures.entry(key).or_insert(RuleFailure {
            object_id,
            description,
        });
    }
}

fn is_pdf_1_4_operator(operator: &str) -> bool {
    matches!(
        operator,
        "q" | "Q"
            | "cm"
            | "w"
            | "J"
            | "j"
            | "M"
            | "d"
            | "ri"
            | "i"
            | "gs"
            | "m"
            | "l"
            | "c"
            | "v"
            | "y"
            | "h"
            | "re"
            | "S"
            | "s"
            | "f"
            | "F"
            | "f*"
            | "B"
            | "B*"
            | "b"
            | "b*"
            | "n"
            | "W"
            | "W*"
            | "BT"
            | "ET"
            | "Tc"
            | "Tw"
            | "Tz"
            | "TL"
            | "Tf"
            | "Tr"
            | "Ts"
            | "Td"
            | "TD"
            | "Tm"
            | "T*"
            | "Tj"
            | "TJ"
            | "'"
            | "\""
            | "d0"
            | "d1"
            | "CS"
            | "cs"
            | "SC"
            | "SCN"
            | "sc"
            | "scn"
            | "G"
            | "g"
            | "RG"
            | "rg"
            | "K"
            | "k"
            | "sh"
            | "BI"
            | "ID"
            | "EI"
            | "Do"
            | "MP"
            | "DP"
            | "BMC"
            | "BDC"
            | "EMC"
            | "BX"
            | "EX"
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ContentToken {
    Name(Vec<u8>),
    Bare(Vec<u8>),
    OpenArray,
    CloseArray,
    Other,
}

fn inline_image_color_space_names(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut names = Vec::new();
    let mut cursor = 0usize;
    let mut array_depth = 0usize;
    while let Some((token, next)) = next_content_token(bytes, cursor) {
        cursor = next;
        match token {
            ContentToken::OpenArray => array_depth = array_depth.saturating_add(1),
            ContentToken::CloseArray => array_depth = array_depth.saturating_sub(1),
            ContentToken::Bare(token) if array_depth == 0 && token == b"BI" => {
                let mut color_space_key = false;
                while let Some((token, next)) = next_content_token(bytes, cursor) {
                    cursor = next;
                    match token {
                        ContentToken::Bare(token) if token == b"ID" => {
                            cursor = find_inline_image_end(bytes, cursor).unwrap_or(bytes.len());
                            break;
                        }
                        ContentToken::Name(name) if color_space_key => {
                            names.push(name);
                            color_space_key = false;
                        }
                        ContentToken::Name(name) => {
                            color_space_key = matches!(name.as_slice(), b"CS" | b"ColorSpace");
                        }
                        _ => color_space_key = false,
                    }
                }
            }
            _ => {}
        }
    }
    names
}

fn inline_image_has_lzw_filter(bytes: &[u8]) -> bool {
    let mut cursor = 0usize;
    let mut array_depth = 0usize;
    while let Some((token, next)) = next_content_token(bytes, cursor) {
        cursor = next;
        match token {
            ContentToken::OpenArray => array_depth = array_depth.saturating_add(1),
            ContentToken::CloseArray => array_depth = array_depth.saturating_sub(1),
            ContentToken::Bare(token) if array_depth == 0 && token == b"BI" => {
                let mut filter_value = false;
                let mut filter_array_depth = None;
                while let Some((token, next)) = next_content_token(bytes, cursor) {
                    cursor = next;
                    match token {
                        ContentToken::Bare(token) if token == b"ID" => {
                            cursor = find_inline_image_end(bytes, cursor).unwrap_or(bytes.len());
                            break;
                        }
                        ContentToken::Name(name) if filter_value => {
                            if is_lzw_filter(&name) {
                                return true;
                            }
                            filter_value = false;
                        }
                        ContentToken::OpenArray if filter_value => {
                            filter_array_depth = Some(1usize);
                            filter_value = false;
                        }
                        ContentToken::OpenArray if filter_array_depth.is_some() => {
                            filter_array_depth = filter_array_depth.map(|depth| depth + 1);
                        }
                        ContentToken::CloseArray if filter_array_depth.is_some() => {
                            filter_array_depth =
                                filter_array_depth.and_then(|depth| depth.checked_sub(1));
                        }
                        ContentToken::Name(name) if filter_array_depth.is_some() => {
                            if is_lzw_filter(&name) {
                                return true;
                            }
                        }
                        ContentToken::Name(name) => {
                            filter_value = matches!(name.as_slice(), b"F" | b"Filter");
                        }
                        _ => filter_value = false,
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Removes inline-image dictionaries and data before using lopdf's content
/// decoder. Inline images are handled above with the bounded tokenizer because
/// `Content::decode` does not safely decode every valid inline-image form.
pub(crate) fn content_without_inline_images(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    let mut cursor = 0usize;
    let mut retained = 0usize;
    let mut array_depth = 0usize;
    while cursor < bytes.len() {
        let token_start = cursor;
        let Some((token, next)) = next_content_token(bytes, cursor) else {
            break;
        };
        cursor = next;
        match token {
            ContentToken::OpenArray => array_depth = array_depth.saturating_add(1),
            ContentToken::CloseArray => array_depth = array_depth.saturating_sub(1),
            ContentToken::Bare(token) if array_depth == 0 && token == b"BI" => {
                while let Some((token, next)) = next_content_token(bytes, cursor) {
                    cursor = next;
                    if matches!(token, ContentToken::Bare(token) if token == b"ID") {
                        cursor = find_inline_image_end(bytes, cursor).unwrap_or(bytes.len());
                        result.extend_from_slice(&bytes[retained..token_start]);
                        result.push(b' ');
                        retained = cursor;
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    result.extend_from_slice(&bytes[retained..]);
    result
}

fn is_lzw_filter(name: &[u8]) -> bool {
    matches!(name, b"LZW" | b"LZWDecode")
}

fn next_content_token(bytes: &[u8], mut cursor: usize) -> Option<(ContentToken, usize)> {
    loop {
        while cursor < bytes.len() && is_pdf_whitespace(bytes[cursor]) {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'%') {
            while cursor < bytes.len() && !matches!(bytes[cursor], b'\n' | b'\r') {
                cursor += 1;
            }
            continue;
        }
        break;
    }
    let byte = *bytes.get(cursor)?;
    match byte {
        b'/' => {
            cursor += 1;
            let start = cursor;
            while cursor < bytes.len() && !is_pdf_delimiter_or_whitespace(bytes[cursor]) {
                cursor += 1;
            }
            Some((
                ContentToken::Name(decode_pdf_name(&bytes[start..cursor])),
                cursor,
            ))
        }
        b'[' => Some((ContentToken::OpenArray, cursor + 1)),
        b']' => Some((ContentToken::CloseArray, cursor + 1)),
        b'(' => {
            cursor += 1;
            let mut depth = 1usize;
            while cursor < bytes.len() && depth > 0 {
                match bytes[cursor] {
                    b'\\' => cursor = cursor.saturating_add(2),
                    b'(' => {
                        depth += 1;
                        cursor += 1;
                    }
                    b')' => {
                        depth -= 1;
                        cursor += 1;
                    }
                    _ => cursor += 1,
                }
            }
            Some((ContentToken::Other, cursor.min(bytes.len())))
        }
        b'<' if bytes.get(cursor + 1) != Some(&b'<') => {
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor] != b'>' {
                cursor += 1;
            }
            Some((ContentToken::Other, (cursor + 1).min(bytes.len())))
        }
        byte if is_pdf_delimiter_or_whitespace(byte) => Some((ContentToken::Other, cursor + 1)),
        _ => {
            let start = cursor;
            while cursor < bytes.len() && !is_pdf_delimiter_or_whitespace(bytes[cursor]) {
                cursor += 1;
            }
            Some((ContentToken::Bare(bytes[start..cursor].to_vec()), cursor))
        }
    }
}

fn find_inline_image_end(bytes: &[u8], cursor: usize) -> Option<usize> {
    bytes
        .get(cursor..)?
        .windows(4)
        .position(|window| {
            is_pdf_whitespace(window[0])
                && window[1] == b'E'
                && window[2] == b'I'
                && is_pdf_whitespace(window[3])
        })
        .map(|offset| cursor + offset + 3)
}

fn decode_pdf_name(bytes: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        if bytes[cursor] == b'#'
            && let Some(pair) = bytes.get(cursor + 1..cursor + 3)
            && let Ok(pair) = std::str::from_utf8(pair)
            && let Ok(byte) = u8::from_str_radix(pair, 16)
        {
            decoded.push(byte);
            cursor += 3;
        } else {
            decoded.push(bytes[cursor]);
            cursor += 1;
        }
    }
    decoded
}

fn is_pdf_delimiter_or_whitespace(byte: u8) -> bool {
    is_pdf_whitespace(byte)
        || matches!(
            byte,
            b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
        )
}

fn is_pdf_whitespace(byte: u8) -> bool {
    matches!(byte, 0 | 9 | 10 | 12 | 13 | 32)
}

fn resource<'a>(
    document: &'a Document,
    limits: &SafetyLimits,
    resources: Option<&'a Dictionary>,
    page_resources: Option<&'a Dictionary>,
    category: &[u8],
    name: &[u8],
) -> Result<Option<&'a Object>, PdfError> {
    if let Some(object) = resource_once(document, limits, resources, category, name)? {
        return Ok(Some(object));
    }
    resource_once(document, limits, page_resources, category, name)
}

#[cfg(test)]
mod tests {
    use lopdf::{Document, Object, dictionary};

    use super::{
        content_without_inline_images, inline_image_color_space_names, inline_image_has_lzw_filter,
        inspect_all_devicen_components,
    };
    use crate::SafetyLimits;

    #[test]
    fn extracts_inline_image_resource_names_without_reading_strings_as_operators() {
        let bytes = b"(BI /CS /Ignored ID x EI) Tj\n\
            BI /W 1 /H 1 /BPC 8 /CS /First ID \0\0\0 EI\n\
            BI /W 1 /H 1 /BPC 8 /ColorSpace /Second ID \0\0\0 EI\n";
        assert_eq!(
            inline_image_color_space_names(bytes),
            vec![b"First".to_vec(), b"Second".to_vec()]
        );
    }

    #[test]
    fn decodes_hex_escapes_in_inline_image_resource_names() {
        assert_eq!(
            inline_image_color_space_names(b"BI /CS /C#53#31 /W 1 /H 1 /BPC 8 ID x EI\n"),
            vec![b"CS1".to_vec()]
        );
    }

    #[test]
    fn recognizes_forbidden_inline_image_lzw_filter_names_and_arrays() {
        assert!(inline_image_has_lzw_filter(
            b"BI /W 1 /H 1 /BPC 8 /F /LZW ID x EI\n"
        ));
        assert!(inline_image_has_lzw_filter(
            b"BI /W 1 /H 1 /BPC 8 /Filter [/AHx /LZWDecode] ID x EI\n"
        ));
        assert!(!inline_image_has_lzw_filter(
            b"BI /W 1 /H 1 /BPC 8 /F /FlateDecode ID x EI\n"
        ));
        assert!(!inline_image_has_lzw_filter(
            b"BI /W 1 /H 1 /BPC 8 /CS /LZW ID x EI\n"
        ));
    }

    #[test]
    fn removes_inline_images_before_lopdf_content_decoding() {
        assert_eq!(
            content_without_inline_images(b"q BI /W 1 /H 1 ID x EI Q"),
            b"q  Q"
        );
    }

    #[test]
    fn finds_oversized_devicen_arrays_outside_executed_content() {
        let mut document = Document::with_version("1.4");
        let components = (0..9)
            .map(|index| Object::Name(format!("Spot{index}").into_bytes()))
            .collect::<Vec<_>>();
        document.add_object(dictionary! {
            "UnusedColorSpace" => vec![
                Object::Name(b"DeviceN".to_vec()),
                Object::Array(components),
                Object::Name(b"DeviceCMYK".to_vec()),
                Object::Null,
            ],
        });
        let failures = inspect_all_devicen_components(&document, &SafetyLimits::default())
            .expect("inspect DeviceN");
        assert_eq!(failures.len(), 1);
    }
}
