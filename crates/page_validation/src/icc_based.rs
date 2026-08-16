use std::collections::{BTreeMap, BTreeSet};

use lopdf::{Document, Object, ObjectId};

use crate::content_support::{ContentExecutionSummary, SelectedColorSpace};
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::IccHeader;
use crate::object_resolution::{ResourceKey, resolve_optional};
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct IccBasedSummary {
    pub(crate) failures: Vec<RuleFailure>,
    pub(crate) failures_pdfa2: Vec<RuleFailure>,
    pub(crate) component_failures: Vec<RuleFailure>,
    pub(crate) device_gray_context: Option<String>,
    pub(crate) device_rgb_context: Option<String>,
    pub(crate) device_cmyk_context: Option<String>,
    pub(crate) invalid_devicen_components: Vec<RuleFailure>,
    pub(crate) invalid_devicen_components_pdfa2: Vec<RuleFailure>,
    pub(crate) invalid_devicen_colorants: Vec<RuleFailure>,
}

#[derive(Clone, Copy, Debug)]
enum DeviceColorSpace {
    Gray,
    Rgb,
    Cmyk,
}

type DeviceNFindings = (Vec<RuleFailure>, Vec<RuleFailure>, Vec<RuleFailure>);

struct Inspector<'a> {
    document: &'a Document,
    limits: &'a SafetyLimits,
    inspected_profiles: BTreeSet<ResourceKey>,
    failures: BTreeMap<ResourceKey, RuleFailure>,
    failures_pdfa2: BTreeMap<ResourceKey, RuleFailure>,
    component_failures: BTreeMap<ResourceKey, RuleFailure>,
    device_gray_context: Option<String>,
    device_rgb_context: Option<String>,
    device_cmyk_context: Option<String>,
    invalid_devicen_components: Vec<RuleFailure>,
    invalid_devicen_components_pdfa2: Vec<RuleFailure>,
    invalid_devicen_colorants: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    execution: &ContentExecutionSummary,
    limits: &SafetyLimits,
) -> Result<IccBasedSummary, PdfError> {
    let mut inspector = Inspector {
        document,
        limits,
        inspected_profiles: BTreeSet::new(),
        failures: BTreeMap::new(),
        failures_pdfa2: BTreeMap::new(),
        component_failures: BTreeMap::new(),
        device_gray_context: None,
        device_rgb_context: None,
        device_cmyk_context: None,
        invalid_devicen_components: Vec::new(),
        invalid_devicen_components_pdfa2: Vec::new(),
        invalid_devicen_colorants: Vec::new(),
    };
    for selected in &execution.selected_color_spaces {
        inspector.inspect_selected_color_space(selected)?;
    }

    // veraPDF exposes PDDeviceN objects independently from colour-space
    // selection, so retain its whole parsed-object population in addition to
    // the executed colour spaces above.
    let (all_pdfa1, all_pdfa2, all_colorants) = inspect_all_devicen_components(document, limits)?;
    inspector.invalid_devicen_components.extend(all_pdfa1);
    inspector.invalid_devicen_components_pdfa2.extend(all_pdfa2);
    inspector.invalid_devicen_colorants.extend(all_colorants);
    inspector.invalid_devicen_components.sort_by(|left, right| {
        left.object_id
            .cmp(&right.object_id)
            .then_with(|| left.description.cmp(&right.description))
    });
    inspector
        .invalid_devicen_components
        .dedup_by(|left, right| {
            left.object_id == right.object_id && left.description == right.description
        });
    inspector
        .invalid_devicen_components_pdfa2
        .sort_by(|left, right| {
            left.object_id
                .cmp(&right.object_id)
                .then_with(|| left.description.cmp(&right.description))
        });
    inspector.invalid_devicen_colorants.sort_by(|left, right| {
        left.object_id
            .cmp(&right.object_id)
            .then_with(|| left.description.cmp(&right.description))
    });
    inspector.invalid_devicen_colorants.dedup_by(|left, right| {
        left.object_id == right.object_id && left.description == right.description
    });
    inspector
        .invalid_devicen_components_pdfa2
        .dedup_by(|left, right| {
            left.object_id == right.object_id && left.description == right.description
        });

    Ok(IccBasedSummary {
        failures: inspector.failures.into_values().collect(),
        failures_pdfa2: inspector.failures_pdfa2.into_values().collect(),
        component_failures: inspector.component_failures.into_values().collect(),
        device_gray_context: inspector.device_gray_context,
        device_rgb_context: inspector.device_rgb_context,
        device_cmyk_context: inspector.device_cmyk_context,
        invalid_devicen_components: inspector.invalid_devicen_components,
        invalid_devicen_components_pdfa2: inspector.invalid_devicen_components_pdfa2,
        invalid_devicen_colorants: inspector.invalid_devicen_colorants,
    })
}

impl Inspector<'_> {
    fn inspect_selected_color_space(
        &mut self,
        selected: &SelectedColorSpace,
    ) -> Result<(), PdfError> {
        self.inspect_color_space(&selected.value, &selected.context)
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
            if components > 32 {
                self.invalid_devicen_components_pdfa2.push(RuleFailure {
                    object_id: value.as_reference().ok().map(Into::into),
                    description: format!(
                        "{context} uses a DeviceN colour space with {components} components"
                    ),
                });
            }
            if !devicen_colorants_present(self.document, items, self.limits)? {
                self.invalid_devicen_colorants.push(RuleFailure {
                    object_id: value.as_reference().ok().map(Into::into),
                    description: format!(
                        "{context} uses a spot colour without a matching DeviceN /Colorants entry"
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
            Some(b"Separation") | Some(b"DeviceN") => {
                if let Some(alternate) = items.get(2) {
                    self.inspect_icc_color_space_at_depth(
                        alternate,
                        &format!("{context}/alternate"),
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
        if !header.conforms_to_pdfa_2_input_profile() {
            self.record_failure_pdfa2(
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
        if !matches!(
            (components, header.color_space.as_str()),
            (Some(1), "GRAY") | (Some(3), "RGB " | "Lab ") | (Some(4), "CMYK")
        ) {
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

    fn record_failure_pdfa2(&mut self, key: ResourceKey, description: String) {
        let object_id = key.object_id();
        self.failures_pdfa2.entry(key).or_insert(RuleFailure {
            object_id,
            description,
        });
    }
}

fn inspect_all_devicen_components(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<DeviceNFindings, PdfError> {
    let mut failures = Vec::new();
    let mut pdfa2_failures = Vec::new();
    let mut colorant_failures = Vec::new();
    let mut visited = BTreeSet::new();
    for (object_id, object) in &document.objects {
        inspect_devicen_object(
            document,
            object,
            Some((*object_id).into()),
            limits,
            &mut visited,
            &mut failures,
            &mut pdfa2_failures,
            &mut colorant_failures,
        )?;
    }
    Ok((failures, pdfa2_failures, colorant_failures))
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
    pdfa2_failures: &mut Vec<RuleFailure>,
    colorant_failures: &mut Vec<RuleFailure>,
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
                    pdfa2_failures,
                    colorant_failures,
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
                if components > 32 {
                    pdfa2_failures.push(RuleFailure {
                        object_id: owner,
                        description: format!(
                            "an object uses a DeviceN colour space with {components} components"
                        ),
                    });
                }
                if !devicen_colorants_present(document, items, limits)? {
                    colorant_failures.push(RuleFailure {
                        object_id: owner,
                        description: "a DeviceN colour space has a spot colour without a matching /Colorants entry".to_owned(),
                    });
                }
            }
            for item in items {
                inspect_devicen_object(
                    document,
                    item,
                    owner,
                    limits,
                    visited,
                    failures,
                    pdfa2_failures,
                    colorant_failures,
                )?;
            }
        }
        Object::Dictionary(dictionary) => {
            for (_, value) in dictionary.iter() {
                inspect_devicen_object(
                    document,
                    value,
                    owner,
                    limits,
                    visited,
                    failures,
                    pdfa2_failures,
                    colorant_failures,
                )?;
            }
        }
        Object::Stream(stream) => {
            for (_, value) in stream.dict.iter() {
                inspect_devicen_object(
                    document,
                    value,
                    owner,
                    limits,
                    visited,
                    failures,
                    pdfa2_failures,
                    colorant_failures,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn devicen_colorants_present(
    document: &Document,
    items: &[Object],
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Some(names) = items
        .get(1)
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_array().ok())
    else {
        return Ok(true);
    };
    let Some(attributes) = items
        .get(4)
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_dict().ok())
    else {
        return Ok(names.iter().all(is_process_colorant));
    };
    let Some(colorants) = attributes
        .get(b"Colorants")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_dict().ok())
    else {
        return Ok(names.iter().all(is_process_colorant));
    };
    Ok(names.iter().all(|name| {
        is_process_colorant(name)
            || name
                .as_name()
                .ok()
                .is_some_and(|name| colorants.get(name).is_ok())
    }))
}

fn is_process_colorant(value: &Object) -> bool {
    matches!(
        value.as_name().ok(),
        Some(b"Cyan" | b"Magenta" | b"Yellow" | b"Black")
    )
}

#[cfg(test)]
mod tests {
    use lopdf::{Document, Object, dictionary};

    use super::inspect_all_devicen_components;
    use crate::SafetyLimits;

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
        let (failures, pdfa2_failures, colorant_failures) =
            inspect_all_devicen_components(&document, &SafetyLimits::default())
                .expect("inspect DeviceN");
        assert_eq!(failures.len(), 1);
        assert!(pdfa2_failures.is_empty());
        assert_eq!(colorant_failures.len(), 1);
    }
}
