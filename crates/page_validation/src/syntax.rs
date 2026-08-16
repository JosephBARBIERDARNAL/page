use std::collections::{BTreeMap, BTreeSet};

use lopdf::xref::XrefEntry;
use lopdf::{Document, Object};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_limits::ObjectLimitsSummary;

const MIN_INTEGER: i128 = -2_147_483_648;
const MAX_INTEGER: i128 = 2_147_483_647;
const MAX_REAL: f64 = 32_767.0;
const MAX_PDFA_2_REAL: f64 = 3.403e38;
const MIN_PDFA_2_REAL: f64 = 1.175e-38;
const MAX_STRING_BYTES: usize = 65_535;
const MAX_PDFA_2_STRING_BYTES: usize = 32_767;
const MAX_NAME_BYTES: usize = 127;
const MAX_ARRAY_ENTRIES: usize = 8_191;
const MAX_DICTIONARY_ENTRIES: usize = 4_095;
const MAX_INDIRECT_OBJECTS: usize = SafetyLimits::PDF_A1_MAX_INDIRECT_OBJECTS;

#[derive(Clone, Debug, Default)]
pub(crate) struct SyntaxSummary {
    pub(crate) header: HeaderSummary,
    pub(crate) object_limits: ObjectLimitsSummary,
    pub(crate) raw_stream_locations: BTreeMap<PdfObjectId, RawStreamLocation>,
    pub(crate) has_odd_hex_string: bool,
    pub(crate) has_non_hex_character: bool,
    pub(crate) has_invalid_xref_subsection_spacing: bool,
    pub(crate) has_invalid_xref_eol: bool,
    pub(crate) has_invalid_indirect_object_syntax: bool,
}

/// The physical positions of a stream in its source indirect object.
///
/// These are collected while the syntax inspector is already walking every
/// xref-addressable object, so downstream raw-stream checks do not need to
/// rediscover streams by searching the whole input for matching bytes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RawStreamLocation {
    pub(crate) data_start: usize,
    pub(crate) endstream: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HeaderSummary {
    pub(crate) offset: usize,
    pub(crate) has_valid_header: bool,
    pub(crate) has_binary_comment: bool,
    pub(crate) has_post_eof_data: bool,
    pub(crate) is_linearized: bool,
    pub(crate) has_first_linearized_trailer_id: bool,
    pub(crate) first_linearized_trailer_id: Option<Vec<u8>>,
    pub(crate) last_trailer_id: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
enum RawValue {
    Null,
    Boolean,
    Integer(i128),
    Real(f64),
    Name(Vec<u8>),
    String {
        decoded_length: usize,
        is_hex: bool,
        hex_count: usize,
        contains_only_hex: bool,
        bytes: Vec<u8>,
    },
    Array(Vec<RawValue>),
    Dictionary(Vec<(Vec<u8>, RawValue)>),
    Reference,
    Other,
}

impl RawValue {
    fn integer(&self) -> Option<i128> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    fn dictionary_value(&self, key: &[u8]) -> Option<&Self> {
        let Self::Dictionary(entries) = self else {
            return None;
        };
        entries
            .iter()
            .rev()
            .find_map(|(candidate, value)| (candidate == key).then_some(value))
    }
}

struct RawParser<'a> {
    bytes: &'a [u8],
    position: usize,
    maximum_depth: usize,
    maximum_nodes: usize,
    nodes: usize,
}

impl<'a> RawParser<'a> {
    fn at(bytes: &'a [u8], position: usize, limits: &SafetyLimits) -> Result<Self, PdfError> {
        if position > bytes.len() {
            return Err(PdfError::Parse(lopdf::Error::InvalidStream(
                "raw object offset is outside the input".to_owned(),
            )));
        }
        Ok(Self {
            bytes,
            position,
            maximum_depth: limits.max_reference_depth,
            maximum_nodes: limits.max_object_count,
            nodes: 0,
        })
    }

    fn parse_value(&mut self, depth: usize) -> Option<RawValue> {
        if depth > self.maximum_depth || self.nodes >= self.maximum_nodes {
            return None;
        }
        self.skip_space_and_comments();
        self.nodes += 1;
        match self.peek()? {
            b'/' => self.parse_name().map(RawValue::Name),
            b'(' => self.parse_literal_string(),
            b'<' if self.bytes.get(self.position + 1) == Some(&b'<') => {
                self.parse_dictionary(depth + 1)
            }
            b'<' => self.parse_hex_string(),
            b'[' => self.parse_array(depth + 1),
            b't' if self.consume_keyword(b"true") => Some(RawValue::Boolean),
            b'f' if self.consume_keyword(b"false") => Some(RawValue::Boolean),
            b'n' if self.consume_keyword(b"null") => Some(RawValue::Null),
            b'+' | b'-' | b'.' | b'0'..=b'9' => self.parse_number_or_reference(),
            _ => {
                self.skip_regular_token();
                Some(RawValue::Other)
            }
        }
    }

    fn parse_dictionary(&mut self, depth: usize) -> Option<RawValue> {
        self.position += 2;
        let mut entries = Vec::new();
        loop {
            self.skip_space_and_comments();
            if self.bytes.get(self.position..self.position + 2) == Some(b">>") {
                self.position += 2;
                return Some(RawValue::Dictionary(entries));
            }
            let key = self.parse_name()?;
            let value = self.parse_value(depth)?;
            entries.push((key, value));
        }
    }

    fn parse_array(&mut self, depth: usize) -> Option<RawValue> {
        self.position += 1;
        let mut values = Vec::new();
        loop {
            self.skip_space_and_comments();
            if self.peek()? == b']' {
                self.position += 1;
                return Some(RawValue::Array(values));
            }
            values.push(self.parse_value(depth)?);
        }
    }

    fn parse_name(&mut self) -> Option<Vec<u8>> {
        (self.peek()? == b'/').then_some(())?;
        self.position += 1;
        let mut decoded = Vec::new();
        while let Some(byte) = self.peek() {
            if is_pdf_whitespace(byte) || is_delimiter(byte) {
                break;
            }
            if byte == b'#'
                && let (Some(high), Some(low)) = (
                    self.bytes
                        .get(self.position + 1)
                        .and_then(|byte| hex(*byte)),
                    self.bytes
                        .get(self.position + 2)
                        .and_then(|byte| hex(*byte)),
                )
            {
                decoded.push((high << 4) | low);
                self.position += 3;
            } else {
                decoded.push(byte);
                self.position += 1;
            }
        }
        Some(decoded)
    }

    fn parse_literal_string(&mut self) -> Option<RawValue> {
        self.position += 1;
        let mut nesting = 1usize;
        let mut decoded = Vec::new();
        while let Some(byte) = self.peek() {
            self.position += 1;
            match byte {
                b'(' => {
                    nesting = nesting.checked_add(1)?;
                    if nesting > self.maximum_depth {
                        return None;
                    }
                    decoded.push(byte);
                }
                b')' => {
                    nesting -= 1;
                    if nesting == 0 {
                        return Some(RawValue::String {
                            decoded_length: decoded.len(),
                            is_hex: false,
                            hex_count: 0,
                            contains_only_hex: true,
                            bytes: decoded,
                        });
                    }
                    decoded.push(byte);
                }
                b'\\' => self.parse_literal_escape(&mut decoded)?,
                b'\r' => {
                    if self.peek() == Some(b'\n') {
                        self.position += 1;
                    }
                    decoded.push(b'\n');
                }
                b'\n' => decoded.push(b'\n'),
                _ => decoded.push(byte),
            }
        }
        None
    }

    fn parse_literal_escape(&mut self, decoded: &mut Vec<u8>) -> Option<()> {
        let byte = self.peek()?;
        self.position += 1;
        match byte {
            b'n' => decoded.push(b'\n'),
            b'r' => decoded.push(b'\r'),
            b't' => decoded.push(b'\t'),
            b'b' => decoded.push(8),
            b'f' => decoded.push(12),
            b'\r' => {
                if self.peek() == Some(b'\n') {
                    self.position += 1;
                }
            }
            b'\n' => {}
            b'0'..=b'7' => {
                let mut value = u16::from(byte - b'0');
                for _ in 0..2 {
                    let Some(next @ b'0'..=b'7') = self.peek() else {
                        break;
                    };
                    self.position += 1;
                    value = value * 8 + u16::from(next - b'0');
                }
                decoded.push(value as u8);
            }
            _ => decoded.push(byte),
        }
        Some(())
    }

    fn parse_hex_string(&mut self) -> Option<RawValue> {
        self.position += 1;
        let mut digits = Vec::new();
        let mut contains_only_hex = true;
        while let Some(byte) = self.peek() {
            self.position += 1;
            if byte == b'>' {
                let hex_count = digits.len();
                let mut decoded = Vec::with_capacity(hex_count.div_ceil(2));
                let mut high = None;
                for digit in &digits {
                    let value = match hex(*digit) {
                        Some(value) => value,
                        None => {
                            contains_only_hex = false;
                            0
                        }
                    };
                    if let Some(high) = high.take() {
                        decoded.push((high << 4) | value);
                    } else {
                        high = Some(value);
                    }
                }
                if let Some(high) = high {
                    decoded.push(high << 4);
                }
                return Some(RawValue::String {
                    decoded_length: decoded.len(),
                    is_hex: true,
                    hex_count,
                    contains_only_hex,
                    bytes: decoded,
                });
            }
            if !is_pdf_whitespace(byte) {
                digits.push(byte);
            }
        }
        None
    }

    fn parse_number_or_reference(&mut self) -> Option<RawValue> {
        let start = self.position;
        let first = self.take_number_token()?;
        let after_first = self.position;
        if !first.contains(&b'.') {
            self.skip_space_and_comments();
            let second_start = self.position;
            if let Some(second) = self.take_unsigned_integer_token() {
                self.skip_space_and_comments();
                if self.consume_keyword(b"R") {
                    let _ = second;
                    return Some(RawValue::Reference);
                }
            }
            self.position = after_first.max(second_start.min(after_first));
        }
        self.position = after_first;
        let token = std::str::from_utf8(first).ok()?;
        if first.contains(&b'.') {
            token.parse::<f64>().ok().map(RawValue::Real)
        } else {
            token.parse::<i128>().ok().map(RawValue::Integer)
        }
        .or_else(|| {
            self.position = start + first.len();
            Some(RawValue::Other)
        })
    }

    fn take_number_token(&mut self) -> Option<&'a [u8]> {
        let start = self.position;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.position += 1;
        }
        let mut has_digit = false;
        let mut has_dot = false;
        while let Some(byte) = self.peek() {
            match byte {
                b'0'..=b'9' => {
                    has_digit = true;
                    self.position += 1;
                }
                b'.' if !has_dot => {
                    has_dot = true;
                    self.position += 1;
                }
                _ => break,
            }
        }
        has_digit.then_some(&self.bytes[start..self.position])
    }

    fn take_unsigned_integer_token(&mut self) -> Option<&'a [u8]> {
        let start = self.position;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.position += 1;
        }
        (self.position > start).then_some(&self.bytes[start..self.position])
    }

    fn skip_space_and_comments(&mut self) {
        loop {
            while self.peek().is_some_and(is_pdf_whitespace) {
                self.position += 1;
            }
            if self.peek() != Some(b'%') {
                break;
            }
            while self
                .peek()
                .is_some_and(|byte| !matches!(byte, b'\r' | b'\n'))
            {
                self.position += 1;
            }
        }
    }

    fn skip_regular_token(&mut self) {
        while self
            .peek()
            .is_some_and(|byte| !is_pdf_whitespace(byte) && !is_delimiter(byte))
        {
            self.position += 1;
        }
    }

    fn consume_keyword(&mut self, keyword: &[u8]) -> bool {
        if self.bytes.get(self.position..self.position + keyword.len()) == Some(keyword)
            && is_pdf_boundary(self.bytes.get(self.position + keyword.len()).copied())
        {
            self.position += keyword.len();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }
}

pub(crate) fn inspect(
    bytes: &[u8],
    document: &Document,
    limits: &SafetyLimits,
) -> Result<SyntaxSummary, PdfError> {
    let revisions = inspect_revisions(bytes, limits)?;
    let mut summary = SyntaxSummary {
        header: inspect_header(bytes, &revisions),
        ..SyntaxSummary::default()
    };

    let mut inspected_ids = BTreeSet::new();
    for (number, entry) in &document.reference_table.entries {
        let XrefEntry::Normal { offset, generation } = entry else {
            continue;
        };
        let object_id = PdfObjectId {
            object_number: *number,
            generation: *generation,
        };
        if !inspected_ids.insert(object_id) {
            continue;
        }
        let adjusted_offset = usize::try_from(*offset)
            .ok()
            .and_then(|offset| offset.checked_add(summary.header.offset))
            .unwrap_or(usize::MAX);
        if let Some(value) = inspect_indirect_object(
            bytes,
            adjusted_offset,
            object_id,
            document,
            limits,
            &mut summary,
        )? {
            collect_value_findings(&value, object_id, &mut summary);
        }
    }

    // Compressed or parser-recovered values have no standalone source object.
    // Retain their normalized limit coverage without double-counting normal
    // objects already inspected from source.
    for (object_id, object) in &document.objects {
        let id: PdfObjectId = (*object_id).into();
        if !inspected_ids.contains(&id) {
            collect_lopdf_value_findings(object, id, &mut summary.object_limits);
        }
    }
    collect_lopdf_dictionary_findings(&document.trailer, None, &mut summary.object_limits);

    let indirect_count = document
        .reference_table
        .entries
        .values()
        .filter(|entry| !matches!(entry, XrefEntry::Free | XrefEntry::UnusableFree))
        .count();
    summary.object_limits.too_many_indirect_objects = indirect_count > MAX_INDIRECT_OBJECTS;

    for revision in &revisions {
        summary.has_invalid_xref_subsection_spacing |= !revision.spacing_compliant;
        summary.has_invalid_xref_eol |= !revision.eol_compliant;
        if let Some(trailer) = &revision.trailer {
            collect_value_findings(
                trailer,
                PdfObjectId {
                    object_number: 0,
                    generation: 0,
                },
                &mut summary,
            );
        }
    }
    Ok(summary)
}

fn inspect_indirect_object(
    bytes: &[u8],
    offset: usize,
    object_id: PdfObjectId,
    document: &Document,
    limits: &SafetyLimits,
    summary: &mut SyntaxSummary,
) -> Result<Option<RawValue>, PdfError> {
    if offset >= bytes.len() {
        return Ok(None);
    }
    let mut parser = RawParser::at(bytes, offset, limits)?;
    let header_start = parser.position;
    let number = parser.take_unsigned_integer_token();
    let first_separator_start = parser.position;
    parser.skip_space_and_comments();
    let generation = parser.take_unsigned_integer_token();
    let second_separator_start = parser.position;
    parser.skip_space_and_comments();
    let obj_start = parser.position;
    let has_obj = parser.consume_keyword(b"obj");
    let header_end = parser.position;
    let valid_header = number.is_some()
        && generation.is_some()
        && first_separator_start + 1
            == second_separator_start.saturating_sub(generation.map_or(0, <[u8]>::len))
        && second_separator_start + 1 == obj_start
        && has_obj
        && is_eol_before(bytes, header_start)
        && single_eol_end(bytes, header_end).is_some();
    summary.has_invalid_indirect_object_syntax |= !valid_header;

    parser.skip_space_and_comments();
    let value = parser.parse_value(0);
    if document
        .objects
        .get(&(object_id.object_number, object_id.generation))
        .is_some_and(|object| object.as_stream().is_ok())
        && let Some(location) = raw_stream_location(bytes, parser.position)
    {
        summary.raw_stream_locations.insert(object_id, location);
    }
    let Some(mut cursor) = find_endobj_start(bytes, parser.position, document, object_id) else {
        summary.has_invalid_indirect_object_syntax = true;
        return Ok(value);
    };
    summary.has_invalid_indirect_object_syntax |= !is_eol_before(bytes, cursor);
    cursor += b"endobj".len();
    summary.has_invalid_indirect_object_syntax |= single_eol_end(bytes, cursor).is_none();
    Ok(value)
}

fn raw_stream_location(bytes: &[u8], after_dictionary: usize) -> Option<RawStreamLocation> {
    let keyword = find_bounded_keyword(bytes, b"stream", after_dictionary)?;
    let data_start = stream_data_start_after_keyword(bytes, keyword + b"stream".len())?;
    Some(RawStreamLocation {
        data_start,
        endstream: find_bounded_keyword(bytes, b"endstream", data_start),
    })
}

fn stream_data_start_after_keyword(bytes: &[u8], mut cursor: usize) -> Option<usize> {
    while matches!(bytes.get(cursor), Some(b' ' | b'\t')) {
        cursor += 1;
    }
    match (bytes.get(cursor), bytes.get(cursor + 1)) {
        (Some(b'\r'), Some(b'\n')) => Some(cursor + 2),
        (Some(b'\r' | b'\n'), _) => Some(cursor + 1),
        _ => None,
    }
}

fn find_endobj_start(
    bytes: &[u8],
    after_value: usize,
    document: &Document,
    object_id: PdfObjectId,
) -> Option<usize> {
    let lopdf_id = (object_id.object_number, object_id.generation);
    let search_start = document
        .objects
        .get(&lopdf_id)
        .and_then(|object| object.as_stream().ok())
        .and_then(|stream| {
            stream
                .start_position
                .and_then(|start| start.checked_add(stream.content.len()))
        })
        .unwrap_or(after_value);
    find_bounded_keyword(bytes, b"endobj", search_start)
}

fn collect_value_findings(value: &RawValue, object_id: PdfObjectId, summary: &mut SyntaxSummary) {
    match value {
        RawValue::Integer(value) if !(*value >= MIN_INTEGER && *value <= MAX_INTEGER) => {
            summary.object_limits.out_of_range_integers.push(object_id);
        }
        RawValue::Real(value)
            if !(value.is_finite() && *value >= -MAX_REAL && *value <= MAX_REAL) =>
        {
            summary.object_limits.out_of_range_reals.push(object_id);
        }
        RawValue::Real(value)
            if !(value.is_finite() && *value >= -MAX_PDFA_2_REAL && *value <= MAX_PDFA_2_REAL) =>
        {
            summary
                .object_limits
                .out_of_range_reals_pdfa_2
                .push(object_id);
        }
        RawValue::Real(value)
            if value.is_finite() && *value != 0.0 && value.abs() < MIN_PDFA_2_REAL =>
        {
            summary.object_limits.underflow_reals_pdfa_2.push(object_id);
        }
        RawValue::Name(value) if value.len() > MAX_NAME_BYTES => {
            summary.object_limits.overlong_names.push(object_id);
        }
        RawValue::String {
            decoded_length,
            is_hex,
            hex_count,
            contains_only_hex,
            ..
        } => {
            if *decoded_length > MAX_STRING_BYTES {
                summary.object_limits.overlong_strings.push(object_id);
            }
            if *decoded_length > MAX_PDFA_2_STRING_BYTES {
                summary
                    .object_limits
                    .overlong_strings_pdfa_2
                    .push(object_id);
            }
            if *is_hex {
                summary.has_odd_hex_string |= hex_count % 2 != 0;
                summary.has_non_hex_character |= !contains_only_hex;
            }
        }
        RawValue::Array(values) => {
            if values.len() > MAX_ARRAY_ENTRIES {
                summary.object_limits.oversized_arrays.push(object_id);
            }
            for value in values {
                collect_value_findings(value, object_id, summary);
            }
        }
        RawValue::Dictionary(entries) => {
            let effective = effective_dictionary(entries);
            if effective
                .values()
                .filter(|value| !matches!(value, RawValue::Null))
                .count()
                > MAX_DICTIONARY_ENTRIES
            {
                summary.object_limits.oversized_dictionaries.push(object_id);
            }
            for value in effective.values() {
                collect_value_findings(value, object_id, summary);
            }
        }
        RawValue::Null
        | RawValue::Boolean
        | RawValue::Reference
        | RawValue::Other
        | RawValue::Integer(_)
        | RawValue::Real(_)
        | RawValue::Name(_) => {}
    }
}

fn effective_dictionary(entries: &[(Vec<u8>, RawValue)]) -> BTreeMap<&[u8], &RawValue> {
    let mut effective = BTreeMap::new();
    for (key, value) in entries {
        effective.insert(key.as_slice(), value);
    }
    effective
}

fn collect_lopdf_value_findings(
    value: &Object,
    object_id: PdfObjectId,
    summary: &mut ObjectLimitsSummary,
) {
    match value {
        Object::Integer(value)
            if !(*value >= MIN_INTEGER as i64 && *value <= MAX_INTEGER as i64) =>
        {
            summary.out_of_range_integers.push(object_id);
        }
        Object::Real(value) if !(*value >= -(MAX_REAL as f32) && *value <= MAX_REAL as f32) => {
            summary.out_of_range_reals.push(object_id);
        }
        Object::Real(value)
            if !(*value >= -(MAX_PDFA_2_REAL as f32) && *value <= MAX_PDFA_2_REAL as f32) =>
        {
            summary.out_of_range_reals_pdfa_2.push(object_id);
        }
        Object::Real(value) if *value != 0.0 && (*value as f64).abs() < MIN_PDFA_2_REAL => {
            summary.underflow_reals_pdfa_2.push(object_id);
        }
        Object::String(value, _) if value.len() > MAX_STRING_BYTES => {
            summary.overlong_strings.push(object_id);
        }
        Object::String(value, _) if value.len() > MAX_PDFA_2_STRING_BYTES => {
            summary.overlong_strings_pdfa_2.push(object_id);
        }
        Object::Name(value) if value.len() > MAX_NAME_BYTES => {
            summary.overlong_names.push(object_id);
        }
        Object::Array(values) => {
            if values.len() > MAX_ARRAY_ENTRIES {
                summary.oversized_arrays.push(object_id);
            }
            for value in values {
                collect_lopdf_value_findings(value, object_id, summary);
            }
        }
        Object::Dictionary(dictionary) => {
            collect_lopdf_dictionary_findings(dictionary, Some(object_id), summary);
        }
        Object::Stream(stream) => {
            collect_lopdf_dictionary_findings(&stream.dict, Some(object_id), summary);
        }
        _ => {}
    }
}

fn collect_lopdf_dictionary_findings(
    dictionary: &lopdf::Dictionary,
    object_id: Option<PdfObjectId>,
    summary: &mut ObjectLimitsSummary,
) {
    let mut meaningful_entries = 0;
    for (_, value) in dictionary.iter() {
        if !matches!(value, Object::Null) {
            meaningful_entries += 1;
        }
        if let Some(object_id) = object_id {
            collect_lopdf_value_findings(value, object_id, summary);
        }
    }
    if meaningful_entries > MAX_DICTIONARY_ENTRIES
        && let Some(object_id) = object_id
    {
        summary.oversized_dictionaries.push(object_id);
    }
}

#[derive(Clone, Debug)]
struct Revision {
    offset: usize,
    spacing_compliant: bool,
    eol_compliant: bool,
    trailer: Option<RawValue>,
    previous: Option<usize>,
    xref_stream: Option<usize>,
}

fn inspect_revisions(bytes: &[u8], limits: &SafetyLimits) -> Result<Vec<Revision>, PdfError> {
    let Some(last) = final_startxref(bytes) else {
        return Ok(Vec::new());
    };
    let mut pending = vec![last];
    let mut seen = BTreeSet::new();
    let mut revisions = Vec::new();
    while let Some(offset) = pending.pop() {
        if revisions.len() >= limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
        }
        if !seen.insert(offset) {
            break;
        }
        let Some(revision) = parse_revision(bytes, offset, limits) else {
            break;
        };
        if let Some(previous) = revision.previous {
            pending.push(previous);
        }
        if let Some(xref_stream) = revision.xref_stream {
            pending.push(xref_stream);
        }
        revisions.push(revision);
    }
    Ok(revisions)
}

fn parse_revision(bytes: &[u8], offset: usize, limits: &SafetyLimits) -> Option<Revision> {
    if bytes.get(offset..offset + b"xref".len()) != Some(b"xref") {
        return Some(Revision {
            offset,
            spacing_compliant: true,
            eol_compliant: true,
            trailer: None,
            previous: None,
            xref_stream: None,
        });
    }
    let mut cursor = offset + b"xref".len();
    let mut eol_compliant = true;
    let mut spacing_compliant = true;
    let eol_count = consume_eols(bytes, &mut cursor);
    eol_compliant &= eol_count == 1;
    skip_horizontal_space(bytes, &mut cursor);

    loop {
        if bytes.get(cursor..cursor + b"trailer".len()) == Some(b"trailer") {
            cursor += b"trailer".len();
            let mut parser = RawParser::at(bytes, cursor, limits).ok()?;
            parser.skip_space_and_comments();
            let trailer = parser.parse_value(0);
            let previous = trailer
                .as_ref()
                .and_then(|value| value.dictionary_value(b"Prev"))
                .and_then(RawValue::integer)
                .and_then(|value| usize::try_from(value).ok());
            let xref_stream = trailer
                .as_ref()
                .and_then(|value| value.dictionary_value(b"XRefStm"))
                .and_then(RawValue::integer)
                .and_then(|value| usize::try_from(value).ok());
            return Some(Revision {
                offset,
                spacing_compliant,
                eol_compliant,
                trailer,
                previous,
                xref_stream,
            });
        }
        let (line, next) = read_line(bytes, cursor)?;
        if line.is_empty() {
            eol_compliant = false;
            cursor = next;
            continue;
        }
        let (count, spacing) = parse_subsection_header(line)?;
        spacing_compliant &= spacing;
        cursor = next;
        for _ in 0..count {
            let (_, next) = read_line(bytes, cursor)?;
            cursor = next;
        }
    }
}

fn parse_subsection_header(line: &[u8]) -> Option<(usize, bool)> {
    let first_end = line.iter().position(|byte| !byte.is_ascii_digit())?;
    let second_start = line[first_end..]
        .iter()
        .position(|byte| byte.is_ascii_digit())
        .map(|offset| first_end + offset)?;
    let second_end = second_start
        + line[second_start..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
    (first_end > 0 && second_end == line.len()).then_some(())?;
    let count = std::str::from_utf8(&line[second_start..second_end])
        .ok()?
        .parse()
        .ok()?;
    Some((count, &line[first_end..second_start] == b" "))
}

fn inspect_header(bytes: &[u8], revisions: &[Revision]) -> HeaderSummary {
    let marker = bytes
        .windows(b"%PDF-".len())
        .position(|window| window == b"%PDF-");
    let header_end = marker.and_then(|start| {
        bytes[start..]
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
            .map(|length| start + length)
    });
    let has_valid_header = marker.zip(header_end).is_some_and(|(start, end)| {
        start == 0
            && end == b"%PDF-1.0".len()
            && bytes[..end].starts_with(b"%PDF-1.")
            && matches!(bytes[7], b'0'..=b'7')
    });
    let comment_start = header_end.and_then(|end| single_eol_end(bytes, end));
    let has_binary_comment = comment_start.is_some_and(|start| {
        bytes.get(start) == Some(&b'%')
            && bytes
                .get(start + 1..start + 5)
                .is_some_and(|comment| comment.iter().all(|byte| *byte > 127))
    });
    let has_post_eof_data = bytes
        .windows(b"%%EOF".len())
        .rposition(|window| window == b"%%EOF")
        .is_none_or(|offset| {
            !matches!(
                &bytes[offset + b"%%EOF".len()..],
                b"" | b"\n" | b"\r" | b"\r\n"
            )
        });

    let linearization = first_indirect_dictionary(bytes).filter(|(_, value)| {
        value
            .dictionary_value(b"Linearized")
            .is_some_and(|value| !matches!(value, RawValue::Null))
            && value.dictionary_value(b"L").and_then(RawValue::integer) == Some(bytes.len() as i128)
    });
    let is_linearized = linearization.is_some_and(|(offset, _)| offset < 1_024);

    let last_revision = revisions.iter().max_by_key(|revision| revision.offset);
    let first_revision = revisions.iter().min_by_key(|revision| revision.offset);
    let last_trailer_id = last_revision
        .and_then(|revision| revision.trailer.as_ref())
        .and_then(trailer_id);
    let first_linearized_trailer_id = is_linearized
        .then(|| {
            first_revision
                .and_then(|revision| revision.trailer.as_ref())
                .and_then(trailer_id)
        })
        .flatten();

    HeaderSummary {
        offset: marker.unwrap_or(0),
        has_valid_header,
        has_binary_comment,
        has_post_eof_data,
        is_linearized,
        has_first_linearized_trailer_id: first_linearized_trailer_id.is_some(),
        first_linearized_trailer_id,
        last_trailer_id,
    }
}

fn first_indirect_dictionary(bytes: &[u8]) -> Option<(usize, RawValue)> {
    let marker = bytes.windows(5).position(|window| window == b"%PDF-")?;
    let mut cursor = bytes[marker..]
        .iter()
        .position(|byte| matches!(byte, b'\r' | b'\n'))
        .map(|offset| marker + offset)?;
    cursor = single_eol_end(bytes, cursor)?;
    while cursor < bytes.len().min(1_024) {
        while bytes.get(cursor).copied().is_some_and(is_pdf_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'%') {
            while bytes
                .get(cursor)
                .is_some_and(|byte| !matches!(byte, b'\r' | b'\n'))
            {
                cursor += 1;
            }
            continue;
        }
        let start = cursor;
        let mut parser = RawParser {
            bytes,
            position: cursor,
            maximum_depth: 128,
            maximum_nodes: 32_768,
            nodes: 0,
        };
        parser.take_unsigned_integer_token()?;
        parser.skip_space_and_comments();
        parser.take_unsigned_integer_token()?;
        parser.skip_space_and_comments();
        if !parser.consume_keyword(b"obj") {
            cursor += 1;
            continue;
        }
        parser.skip_space_and_comments();
        let value = parser.parse_value(0)?;
        return matches!(value, RawValue::Dictionary(_)).then_some((start, value));
    }
    None
}

fn trailer_id(trailer: &RawValue) -> Option<Vec<u8>> {
    let RawValue::Array(values) = trailer.dictionary_value(b"ID")? else {
        return None;
    };
    let mut result = Vec::new();
    for value in values {
        if let RawValue::String { bytes, .. } = value {
            result.extend_from_slice(bytes);
        }
    }
    Some(result)
}

pub(crate) fn repair_for_lopdf(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut repaired = bytes.to_vec();
    let mut changed = repair_xref_syntax(&mut repaired);
    changed |= repair_hex_strings(&mut repaired);
    changed.then_some(repaired)
}

fn repair_xref_syntax(bytes: &mut [u8]) -> bool {
    let Some(mut cursor) = final_startxref(bytes) else {
        return false;
    };
    let mut changed = false;
    let mut seen = BTreeSet::new();
    while cursor < bytes.len() && seen.insert(cursor) {
        if bytes.get(cursor..cursor + b"xref".len()) != Some(b"xref") {
            break;
        }
        let after_keyword = cursor + b"xref".len();
        if bytes.get(after_keyword) == Some(&b'\n') && bytes.get(after_keyword + 1) == Some(&b'\n')
        {
            bytes[after_keyword] = b' ';
            changed = true;
        }
        let mut line_start = after_keyword;
        while line_start < bytes.len() && bytes[line_start] != b'\n' {
            line_start += 1;
        }
        line_start += usize::from(line_start < bytes.len());
        loop {
            let Some((line, next)) = read_line(bytes, line_start) else {
                return changed;
            };
            if line == b"trailer" {
                let mut parser = RawParser {
                    bytes,
                    position: next,
                    maximum_depth: 128,
                    maximum_nodes: 32_768,
                    nodes: 0,
                };
                parser.skip_space_and_comments();
                let trailer = parser.parse_value(0);
                cursor = trailer
                    .as_ref()
                    .and_then(|value| value.dictionary_value(b"Prev"))
                    .and_then(RawValue::integer)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(bytes.len());
                break;
            }
            if line.is_empty() {
                line_start = next;
                continue;
            }
            let Some(first_end) = line.iter().position(|byte| !byte.is_ascii_digit()) else {
                return changed;
            };
            let Some(second_start) = line[first_end..]
                .iter()
                .position(|byte| byte.is_ascii_digit())
                .map(|offset| first_end + offset)
            else {
                return changed;
            };
            let absolute = line_start + first_end;
            let separator_length = second_start - first_end;
            if separator_length > 0 && &line[first_end..second_start] != b" " {
                for byte in &mut bytes[absolute..absolute + separator_length - 1] {
                    *byte = b'0';
                }
                bytes[absolute + separator_length - 1] = b' ';
                changed = true;
            }
            let Some((count, _)) = parse_subsection_header(&bytes[line_start..next - 1]) else {
                return changed;
            };
            line_start = next;
            for _ in 0..count {
                let Some((_, next)) = read_line(bytes, line_start) else {
                    return changed;
                };
                line_start = next;
            }
        }
    }
    changed
}

fn repair_hex_strings(bytes: &mut [u8]) -> bool {
    let mut changed = false;
    let mut cursor = 0;
    let mut literal_depth = 0usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'%' if literal_depth == 0 => {
                while cursor < bytes.len() && !matches!(bytes[cursor], b'\r' | b'\n') {
                    cursor += 1;
                }
            }
            b'(' => {
                literal_depth += 1;
                cursor += 1;
            }
            b')' if literal_depth > 0 => {
                literal_depth -= 1;
                cursor += 1;
            }
            b'\\' if literal_depth > 0 => cursor = cursor.saturating_add(2),
            b'<' if literal_depth == 0 && bytes.get(cursor + 1) == Some(&b'<') => cursor += 2,
            b'<' if literal_depth == 0 => {
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor] != b'>' {
                    if !is_pdf_whitespace(bytes[cursor]) && hex(bytes[cursor]).is_none() {
                        bytes[cursor] = b'0';
                        changed = true;
                    }
                    cursor += 1;
                }
                cursor += usize::from(cursor < bytes.len());
            }
            b's' if literal_depth == 0
                && bytes.get(cursor..cursor + b"stream".len()) == Some(b"stream")
                && is_pdf_boundary(bytes.get(cursor.wrapping_sub(1)).copied())
                && is_pdf_boundary(bytes.get(cursor + b"stream".len()).copied()) =>
            {
                let Some(end) = find_bounded_keyword(bytes, b"endstream", cursor + b"stream".len())
                else {
                    return changed;
                };
                cursor = end + b"endstream".len();
            }
            _ => cursor += 1,
        }
    }
    changed
}

fn final_startxref(bytes: &[u8]) -> Option<usize> {
    let eof = bytes
        .windows(b"%%EOF".len())
        .rposition(|window| window == b"%%EOF")?;
    let start = bytes[..eof]
        .windows(b"startxref".len())
        .rposition(|window| window == b"startxref")?
        + b"startxref".len();
    let mut cursor = start;
    while bytes.get(cursor).copied().is_some_and(is_pdf_whitespace) {
        cursor += 1;
    }
    let end = bytes[cursor..]
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .map(|length| cursor + length)
        .unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[cursor..end]).ok()?.parse().ok()
}

fn read_line(bytes: &[u8], start: usize) -> Option<(&[u8], usize)> {
    let end = bytes[start..]
        .iter()
        .position(|byte| matches!(byte, b'\r' | b'\n'))
        .map(|offset| start + offset)?;
    Some((&bytes[start..end], single_eol_end(bytes, end)?))
}

fn consume_eols(bytes: &[u8], cursor: &mut usize) -> usize {
    let mut count = 0;
    while let Some(next) = single_eol_end(bytes, *cursor) {
        *cursor = next;
        count += 1;
    }
    count
}

fn skip_horizontal_space(bytes: &[u8], cursor: &mut usize) {
    while matches!(bytes.get(*cursor), Some(b' ' | b'\t' | b'\0' | b'\x0c')) {
        *cursor += 1;
    }
}

fn single_eol_end(bytes: &[u8], cursor: usize) -> Option<usize> {
    match bytes.get(cursor) {
        Some(b'\n') => Some(cursor + 1),
        Some(b'\r') if bytes.get(cursor + 1) == Some(&b'\n') => Some(cursor + 2),
        Some(b'\r') => Some(cursor + 1),
        _ => None,
    }
}

fn find_bounded_keyword(bytes: &[u8], keyword: &[u8], start: usize) -> Option<usize> {
    bytes
        .get(start..)?
        .windows(keyword.len())
        .enumerate()
        .find_map(|(offset, window)| {
            let position = start + offset;
            (window == keyword
                && is_pdf_boundary(bytes.get(position.wrapping_sub(1)).copied())
                && is_pdf_boundary(bytes.get(position + keyword.len()).copied()))
            .then_some(position)
        })
}

fn is_eol_before(bytes: &[u8], cursor: usize) -> bool {
    cursor == 0 || matches!(bytes.get(cursor.wrapping_sub(1)), Some(b'\r' | b'\n'))
}

fn is_pdf_boundary(byte: Option<u8>) -> bool {
    byte.is_none_or(|byte| is_pdf_whitespace(byte) || is_delimiter(byte))
}

fn is_pdf_whitespace(byte: u8) -> bool {
    matches!(byte, b'\0' | b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

fn is_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(bytes: &[u8]) -> RawValue {
        let mut parser = RawParser {
            bytes,
            position: 0,
            maximum_depth: 128,
            maximum_nodes: 100_000,
            nodes: 0,
        };
        parser.parse_value(0).expect("raw value")
    }

    #[test]
    fn preserves_duplicate_dictionary_entries_and_uses_the_last_value() {
        let value = parse(b"<< /K 1 /K null /Other 2 >>");
        assert!(matches!(value.dictionary_value(b"K"), Some(RawValue::Null)));
        assert_eq!(
            value.dictionary_value(b"Other").and_then(RawValue::integer),
            Some(2)
        );
    }

    #[test]
    fn decodes_name_escapes_before_measuring() {
        assert!(matches!(parse(b"/A#42C"), RawValue::Name(value) if value == b"ABC"));
    }

    #[test]
    fn retains_invalid_and_odd_hex_provenance() {
        assert!(matches!(
            parse(b"<A G>"),
            RawValue::String {
                is_hex: true,
                hex_count: 2,
                contains_only_hex: false,
                ..
            }
        ));
        assert!(matches!(
            parse(b"<ABC>"),
            RawValue::String {
                decoded_length: 2,
                hex_count: 3,
                ..
            }
        ));
    }

    #[test]
    fn trailer_ids_follow_the_pinned_concat_and_wrong_type_recovery() {
        let trailer = parse(b"<< /ID [(one) 42 <74776f>] >>");
        assert_eq!(trailer_id(&trailer), Some(b"onetwo".to_vec()));
        assert_eq!(trailer_id(&parse(b"<< /ID [] >>")), Some(Vec::new()));
    }

    #[test]
    fn pdfa_2_and_3_header_accepts_only_pdf_1_0_through_1_7() {
        let revisions = [];
        for version in 0..=7 {
            let mut bytes = format!("%PDF-1.{version}\n%").into_bytes();
            bytes.extend_from_slice(&[128, 129, 130, 131, b'\n']);
            assert!(inspect_header(&bytes, &revisions).has_valid_header);
        }
        for header in [b"%PDF-1.8\n".as_slice(), b"%PDF-2.0\n", b"%PDF-1.7 extra\n"] {
            assert!(
                !inspect_header(header, &revisions).has_valid_header,
                "{header:?}"
            );
        }
    }

    #[test]
    fn recognizes_the_pinned_linearized_fixture_and_its_trailers() {
        let bytes = include_bytes!("../tests/fixtures/linearized-baseline.pdf");
        let revisions = inspect_revisions(bytes, &SafetyLimits::default()).unwrap();
        let header = inspect_header(bytes, &revisions);
        assert!(header.is_linearized);
        assert!(header.has_first_linearized_trailer_id);
        assert_eq!(header.first_linearized_trailer_id, header.last_trailer_id);

        let mismatch = include_bytes!("../tests/fixtures/linearized-id-mismatch.pdf");
        let revisions = inspect_revisions(mismatch, &SafetyLimits::default()).unwrap();
        let header = inspect_header(mismatch, &revisions);
        assert!(header.is_linearized);
        assert_ne!(header.first_linearized_trailer_id, header.last_trailer_id);
    }

    #[test]
    fn revision_inspection_preserves_the_configured_depth_limit() {
        let first = b"xref\n0 0\ntrailer\n<<>>\n";
        let second_offset = first.len();
        let bytes = [
            first.as_slice(),
            format!("xref\n0 0\ntrailer\n<< /Prev 0 >>\nstartxref\n{second_offset}\n%%EOF\n")
                .as_bytes(),
        ]
        .concat();
        let limits = SafetyLimits {
            max_reference_depth: 1,
            ..SafetyLimits::default()
        };

        assert!(matches!(
            inspect_revisions(&bytes, &limits),
            Err(PdfError::ReferenceDepth(1))
        ));
    }
}
