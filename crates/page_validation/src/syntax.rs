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
    pub(crate) declared_length: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HeaderSummary {
    pub(crate) offset: usize,
    pub(crate) has_valid_header: bool,
    pub(crate) has_valid_pdfa23_header: bool,
    pub(crate) has_binary_comment: bool,
    pub(crate) has_post_eof_data: bool,
    pub(crate) is_linearized: bool,
    pub(crate) has_first_linearized_trailer_id: bool,
    pub(crate) first_linearized_trailer_id: Option<Vec<u8>>,
    pub(crate) last_trailer_id: Option<Vec<u8>>,
}

/// Bounds the number of objects that the full `lopdf` load may materialize.
///
/// `lopdf` applies its xref table and object parsing before this crate can
/// inspect `Document::objects`, so the configured object limit cannot be the
/// first line of defense on its own. This preflight only keeps the bounded raw
/// syntax state needed to count xref entries and object headers; it does not
/// construct PDF objects.
pub(crate) fn preflight_object_limit(bytes: &[u8], limits: &SafetyLimits) -> Result<(), PdfError> {
    let mut preflight_limits = limits.clone();
    preflight_limits.max_object_count = preflight_limits.max_object_count.max(1_024);
    let revisions = inspect_revisions(bytes, &preflight_limits)?;
    if revisions.iter().any(|revision| {
        revision
            .trailer
            .as_ref()
            .and_then(|trailer| trailer.dictionary_value(b"Encrypt"))
            .is_some()
    }) || trailer_declares_encryption(bytes, &preflight_limits)
    {
        return Ok(());
    }

    let xref_count = revisions.iter().fold(0usize, |count, revision| {
        count.saturating_add(revision.object_count)
    });
    let object_header_count = count_indirect_object_headers(bytes, limits.max_object_count);
    let actual = xref_count.max(object_header_count);
    if actual > limits.max_object_count {
        return Err(PdfError::TooManyObjects {
            actual,
            limit: limits.max_object_count,
        });
    }
    Ok(())
}

fn trailer_declares_encryption(bytes: &[u8], limits: &SafetyLimits) -> bool {
    let Some(start) = final_startxref(bytes) else {
        return false;
    };
    let Some(relative_trailer) = bytes.get(start..).and_then(|tail| {
        tail.windows(b"trailer".len())
            .rposition(|window| window == b"trailer")
    }) else {
        return false;
    };
    let mut parser = RawParser::at(bytes, start + relative_trailer + b"trailer".len(), limits).ok();
    parser.as_mut().is_some_and(|parser| {
        parser.skip_space_and_comments();
        parser
            .parse_value(0)
            .is_some_and(|trailer| trailer.dictionary_value(b"Encrypt").is_some())
    })
}

fn count_indirect_object_headers(bytes: &[u8], limit: usize) -> usize {
    let mut count = 0usize;
    let mut cursor = 0;
    while cursor < bytes.len() {
        if let Some((_, _, next)) = indirect_object_header(bytes, cursor) {
            count = count.saturating_add(1);
            if count > limit {
                return count;
            }
            cursor = stream_end_after_object(bytes, next).unwrap_or(next);
        } else {
            cursor = read_line(bytes, cursor).map_or(bytes.len(), |(_, next)| next);
        }
    }
    count
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
        has_digit.then_some(self.bytes.get(start..self.position)?)
    }

    fn take_unsigned_integer_token(&mut self) -> Option<&'a [u8]> {
        let start = self.position;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.position += 1;
        }
        (self.position > start).then_some(self.bytes.get(start..self.position)?)
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
    parser.skip_space_and_comments();
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
    let value_start = parser.position;
    let value = parser.parse_value(0);
    if let Some(location) = raw_stream_location(bytes, value_start, document) {
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

fn raw_stream_location(
    bytes: &[u8],
    after_dictionary: usize,
    document: &Document,
) -> Option<RawStreamLocation> {
    let endobj = find_bounded_keyword(bytes, b"endobj", after_dictionary)?;
    let keyword = find_bounded_keyword(bytes.get(..endobj)?, b"stream", after_dictionary)?;
    let data_start = stream_data_start_after_keyword(bytes, keyword + b"stream".len())?;
    Some(RawStreamLocation {
        data_start,
        endstream: find_bounded_keyword(bytes, b"endstream", data_start),
        declared_length: raw_stream_declared_length(bytes, after_dictionary, keyword, document),
    })
}

fn raw_stream_declared_length(
    bytes: &[u8],
    dictionary_start: usize,
    stream_keyword: usize,
    document: &Document,
) -> Option<usize> {
    let dictionary = bytes.get(dictionary_start..stream_keyword)?;
    let length_key = dictionary
        .windows(b"/Length".len())
        .enumerate()
        .rev()
        .find_map(|(offset, window)| {
            (window == b"/Length"
                && is_pdf_boundary(dictionary.get(offset + b"/Length".len()).copied()))
            .then_some(offset)
        })?;
    let mut cursor = length_key + b"/Length".len();
    while dictionary
        .get(cursor)
        .copied()
        .is_some_and(is_pdf_whitespace)
    {
        cursor += 1;
    }
    let number_start = cursor;
    while dictionary
        .get(cursor)
        .copied()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        cursor += 1;
    }
    let first = std::str::from_utf8(dictionary.get(number_start..cursor)?)
        .ok()?
        .parse::<usize>()
        .ok()?;
    let mut after_first = cursor;
    while dictionary
        .get(after_first)
        .copied()
        .is_some_and(is_pdf_whitespace)
    {
        after_first += 1;
    }
    let second_start = after_first;
    while dictionary
        .get(after_first)
        .copied()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        after_first += 1;
    }
    if second_start != after_first {
        let mut after_second = after_first;
        while dictionary
            .get(after_second)
            .copied()
            .is_some_and(is_pdf_whitespace)
        {
            after_second += 1;
        }
        if dictionary
            .get(after_second..)
            .is_some_and(|tail| tail.starts_with(b"R"))
        {
            let object_number = u32::try_from(first).ok()?;
            let generation = std::str::from_utf8(dictionary.get(second_start..after_first)?)
                .ok()?
                .parse::<u16>()
                .ok()?;
            return document
                .objects
                .get(&(object_number, generation))
                .and_then(|object| object.as_i64().ok())
                .and_then(|value| usize::try_from(value).ok());
        }
    }
    (number_start != cursor).then_some(first)
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
        RawValue::Real(value) => {
            if !(value.is_finite() && *value >= -MAX_REAL && *value <= MAX_REAL) {
                summary.object_limits.out_of_range_reals.push(object_id);
            }
            if !(value.is_finite() && *value >= -MAX_PDFA_2_REAL && *value <= MAX_PDFA_2_REAL) {
                summary
                    .object_limits
                    .out_of_range_reals_pdfa_2
                    .push(object_id);
            }
            if value.is_finite() && *value != 0.0 && value.abs() < MIN_PDFA_2_REAL {
                summary.object_limits.underflow_reals_pdfa_2.push(object_id);
            }
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
        Object::Real(value) => {
            if !(*value >= -(MAX_REAL as f32) && *value <= MAX_REAL as f32) {
                summary.out_of_range_reals.push(object_id);
            }
            if !(*value >= -(MAX_PDFA_2_REAL as f32) && *value <= MAX_PDFA_2_REAL as f32) {
                summary.out_of_range_reals_pdfa_2.push(object_id);
            }
            if *value != 0.0 && (*value as f64).abs() < MIN_PDFA_2_REAL {
                summary.underflow_reals_pdfa_2.push(object_id);
            }
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
    object_count: usize,
}

fn inspect_revisions(bytes: &[u8], limits: &SafetyLimits) -> Result<Vec<Revision>, PdfError> {
    let Some(last) = final_startxref(bytes) else {
        return Ok(Vec::new());
    };
    let mut pending = vec![last];
    let mut seen = BTreeSet::new();
    let mut revisions = Vec::new();
    while let Some(offset) = pending.pop() {
        if revisions.len() >= limits.max_xref_revisions {
            return Err(PdfError::ReferenceDepth(limits.max_xref_revisions));
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
        let trailer = parse_xref_stream_dictionary(bytes, offset, limits);
        let previous = trailer
            .as_ref()
            .and_then(|value| value.dictionary_value(b"Prev"))
            .and_then(RawValue::integer)
            .and_then(|value| usize::try_from(value).ok());
        let object_count = trailer.as_ref().map_or(0, xref_stream_object_count);
        return Some(Revision {
            offset,
            spacing_compliant: true,
            eol_compliant: true,
            trailer,
            previous,
            xref_stream: None,
            object_count,
        });
    }
    let mut cursor = offset + b"xref".len();
    let mut eol_compliant = true;
    let mut spacing_compliant = true;
    let mut object_count = 0usize;
    let eol_count = consume_eols(bytes, &mut cursor);
    eol_compliant &= eol_count == 1;
    let before_horizontal_space = cursor;
    skip_horizontal_space(bytes, &mut cursor);
    eol_compliant &= cursor == before_horizontal_space;

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
                object_count,
            });
        }
        let (line, next) = read_line(bytes, cursor)?;
        if line.is_empty() {
            cursor = next;
            continue;
        }
        let (count, spacing) = parse_subsection_header(line)?;
        spacing_compliant &= spacing;
        cursor = next;
        for _ in 0..count {
            let (entry, next) = read_line(bytes, cursor)?;
            if xref_entry_is_normal(entry) {
                object_count = object_count.saturating_add(1);
            }
            cursor = next;
        }
    }
}

fn xref_entry_is_normal(line: &[u8]) -> bool {
    line.split(|byte| byte.is_ascii_whitespace())
        .filter(|field| !field.is_empty())
        .nth(2)
        == Some(b"n")
}

fn xref_stream_object_count(value: &RawValue) -> usize {
    let Some(index) = value.dictionary_value(b"Index") else {
        return value
            .dictionary_value(b"Size")
            .and_then(RawValue::integer)
            .and_then(|size| usize::try_from(size).ok())
            .unwrap_or_default()
            .saturating_sub(1);
    };
    let RawValue::Array(values) = index else {
        return 0;
    };
    let (ranges, _) = values.as_chunks::<2>();
    let includes_object_zero = ranges.iter().any(|range| range[0].integer() == Some(0));
    ranges
        .iter()
        .filter_map(|range| {
            let start = range[0].integer()?;
            let count = range[1].integer()?;
            let _ = usize::try_from(start).ok()?;
            usize::try_from(count).ok()
        })
        .fold(0usize, usize::saturating_add)
        .saturating_sub(usize::from(includes_object_zero))
}

fn parse_xref_stream_dictionary(
    bytes: &[u8],
    offset: usize,
    limits: &SafetyLimits,
) -> Option<RawValue> {
    let mut parser = RawParser::at(bytes, offset, limits).ok()?;
    parser.take_unsigned_integer_token()?;
    parser.skip_space_and_comments();
    parser.take_unsigned_integer_token()?;
    parser.skip_space_and_comments();
    parser.consume_keyword(b"obj").then_some(())?;
    parser.parse_value(0).filter(|value| {
        matches!(
            value.dictionary_value(b"Type"),
            Some(RawValue::Name(name)) if name == b"XRef"
        )
    })
}

fn parse_subsection_header(line: &[u8]) -> Option<(usize, bool)> {
    let first_end = line.iter().position(|byte| !byte.is_ascii_digit())?;
    let second_start = line
        .get(first_end..)?
        .iter()
        .position(|byte| byte.is_ascii_digit())
        .map(|offset| first_end + offset)?;
    let second_end = second_start
        + line
            .get(second_start..)?
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
    (first_end > 0 && second_end == line.len()).then_some(())?;
    let count = std::str::from_utf8(line.get(second_start..second_end)?)
        .ok()?
        .parse()
        .ok()?;
    Some((count, line.get(first_end..second_start)? == b" "))
}

fn inspect_header(bytes: &[u8], revisions: &[Revision]) -> HeaderSummary {
    let marker = bytes
        .windows(b"%PDF-".len())
        .position(|window| window == b"%PDF-");
    let header_end = marker.and_then(|start| {
        bytes
            .get(start..)?
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
            .map(|length| start + length)
    });
    let has_valid_header = marker.zip(header_end).is_some_and(|(start, end)| {
        start == 0
            && end == b"%PDF-1.0".len()
            && bytes
                .get(..end)
                .is_some_and(|header| header.starts_with(b"%PDF-"))
            && bytes.get(5).is_some_and(u8::is_ascii_digit)
            && bytes.get(6) == Some(&b'.')
            && bytes.get(7).is_some_and(u8::is_ascii_digit)
    });
    let has_valid_pdfa23_header = marker.zip(header_end).is_some_and(|(start, end)| {
        start == 0
            && end == b"%PDF-1.0".len()
            && bytes
                .get(..end)
                .is_some_and(|header| header.starts_with(b"%PDF-"))
            && bytes.get(5) == Some(&b'1')
            && bytes.get(6) == Some(&b'.')
            && bytes.get(7).is_some_and(u8::is_ascii_digit)
            && bytes.get(7).is_some_and(|byte| *byte <= b'7')
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
                bytes.get(offset + b"%%EOF".len()..).unwrap_or_default(),
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
        has_valid_pdfa23_header,
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
    let mut cursor = bytes
        .get(marker..)?
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
    let header_offset = bytes
        .windows(b"%PDF-".len())
        .position(|window| window == b"%PDF-")
        .filter(|offset| *offset > 0);
    let mut changed = repair_header_syntax(&mut repaired);
    if let Some(header_offset) = header_offset {
        changed |= repair_offsets_for_header(&mut repaired, header_offset);
    }
    changed |= repair_startxref_whitespace(&mut repaired);
    changed |= repair_xref_entry_eols(&mut repaired);
    changed |= repair_xref_syntax(&mut repaired);
    changed |= repair_xref_offsets(&mut repaired);
    changed |= repair_hex_strings(&mut repaired);
    changed.then_some(repaired)
}

fn repair_xref_entry_eols(bytes: &mut Vec<u8>) -> bool {
    let Some(xref_start) = final_startxref(bytes) else {
        return false;
    };
    if bytes.get(xref_start..xref_start + b"xref".len()) != Some(b"xref") {
        return false;
    }
    let mut cursor = xref_start + b"xref".len();
    let mut changed = false;
    while let Some((line, next)) = read_line(bytes, cursor) {
        if line == b"trailer" {
            break;
        }
        let fields = line
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        let is_crlf = bytes.get(next.saturating_sub(2)..next) == Some(b"\r\n");
        if fields.len() >= 3
            && fields
                .get(2)
                .is_some_and(|field| matches!(*field, b"n" | b"f"))
            && !line.ends_with(b" ")
            && !is_crlf
        {
            let eol_start = next.saturating_sub(1);
            bytes.insert(eol_start, b' ');
            changed = true;
            cursor = next + 1;
        } else {
            cursor = next;
        }
    }
    if !changed {
        return false;
    }
    let Some((number_start, number_end, _)) = final_startxref_parts(bytes) else {
        return true;
    };
    let replacement = xref_start.to_string();
    bytes.splice(number_start..number_end, replacement.bytes());
    true
}

fn repair_xref_offsets(bytes: &mut [u8]) -> bool {
    let mut object_offsets = std::collections::HashMap::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let Some((object, generation, next)) = indirect_object_header(bytes, cursor) else {
            cursor = read_line(bytes, cursor).map_or(bytes.len(), |(_, next)| next);
            continue;
        };
        {
            object_offsets.insert((object, generation), cursor);
        }
        cursor = stream_end_after_object(bytes, next).unwrap_or(next);
    }

    let mut changed = false;
    let mut in_xref = false;
    let mut next_object: Option<u32> = None;
    let mut next_generation = 0;
    let mut cursor = 0;
    while cursor < bytes.len() {
        let line_end = bytes
            .get(cursor..)
            .unwrap_or_default()
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
            .map_or(bytes.len(), |offset| cursor + offset);
        let line = bytes.get_mut(cursor..line_end).unwrap_or_default();
        let fields = line
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        if line == b"xref" {
            in_xref = true;
            next_object = None;
        } else if line == b"trailer" {
            in_xref = false;
            next_object = None;
        } else if in_xref {
            if fields.len() == 2
                && fields.iter().all(|field| {
                    std::str::from_utf8(field)
                        .ok()
                        .is_some_and(|field| field.parse::<u32>().is_ok())
                })
            {
                next_object = std::str::from_utf8(fields.first().copied().unwrap_or_default())
                    .ok()
                    .and_then(|field| field.parse::<u32>().ok());
                next_generation = std::str::from_utf8(fields.get(1).copied().unwrap_or_default())
                    .ok()
                    .and_then(|field| field.parse::<u16>().ok())
                    .unwrap_or_default();
            } else if fields.len() >= 3 {
                let Some(entry_type) = fields.get(2) else {
                    cursor = line_end.saturating_add(1);
                    continue;
                };
                if matches!(*entry_type, b"n" | b"f")
                    && let Some(object) = next_object
                {
                    if *entry_type == b"n"
                        && line.len() >= 10
                        && let Some(offset) = object_offsets.get(&(object, next_generation))
                    {
                        let replacement = format!("{offset:010}");
                        if line.get(..10) != Some(replacement.as_bytes())
                            && let Some(prefix) = line.get_mut(..10)
                        {
                            prefix.copy_from_slice(replacement.as_bytes());
                            changed = true;
                        }
                    }
                    next_object = object.checked_add(1);
                }
            }
        }
        cursor = line_end.saturating_add(1);
    }
    changed
}

fn indirect_object_header(bytes: &[u8], cursor: usize) -> Option<(u32, u16, usize)> {
    let (line, next) = read_line(bytes, cursor)?;
    let mut parser = RawParser {
        bytes: line,
        position: 0,
        maximum_depth: 0,
        maximum_nodes: 0,
        nodes: 0,
    };
    let object = std::str::from_utf8(parser.take_unsigned_integer_token()?)
        .ok()?
        .parse()
        .ok()?;
    let after_object = parser.position;
    parser.skip_space_and_comments();
    (parser.position > after_object).then_some(())?;
    let generation = std::str::from_utf8(parser.take_unsigned_integer_token()?)
        .ok()?
        .parse()
        .ok()?;
    let after_generation = parser.position;
    parser.skip_space_and_comments();
    (parser.position > after_generation).then_some(())?;
    parser.consume_keyword(b"obj").then_some(())?;
    parser.skip_space_and_comments();
    (parser.position == line.len()).then_some((object, generation, next))
}

fn stream_end_after_object(bytes: &[u8], after_header: usize) -> Option<usize> {
    let mut parser = RawParser {
        bytes,
        position: after_header,
        maximum_depth: 128,
        maximum_nodes: 32_768,
        nodes: 0,
    };
    let value = parser.parse_value(0)?;
    let RawValue::Dictionary(dictionary) = value else {
        return None;
    };
    parser.skip_space_and_comments();
    parser.consume_keyword(b"stream").then_some(())?;
    let data_start = stream_data_start_after_keyword(bytes, parser.position)?;
    let declared_length = dictionary
        .iter()
        .rev()
        .find_map(|(key, value)| (key == b"Length").then_some(value))
        .and_then(RawValue::integer)
        .and_then(|length| usize::try_from(length).ok());
    let endstream = declared_length
        .and_then(|length| data_start.checked_add(length))
        .and_then(|data_end| find_bounded_keyword(bytes, b"endstream", data_end))
        .or_else(|| find_bounded_keyword(bytes, b"endstream", data_start));
    Some(endstream.map_or(bytes.len(), |offset| offset + b"endstream".len()))
}

fn repair_offsets_for_header(bytes: &mut Vec<u8>, header_offset: usize) -> bool {
    let mut changed = false;
    let mut in_xref = false;
    let mut cursor = 0;
    while cursor < bytes.len() {
        let line_end = bytes
            .get(cursor..)
            .unwrap_or_default()
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
            .map_or(bytes.len(), |offset| cursor + offset);
        let Some(line) = bytes.get_mut(cursor..line_end) else {
            break;
        };
        if line == b"xref" {
            in_xref = true;
        } else if line == b"trailer" {
            in_xref = false;
        } else {
            let xref_offset = if in_xref
                && line.len() >= 18
                && line.get(10) == Some(&b' ')
                && line.get(16) == Some(&b' ')
                && line.get(17).is_some_and(|byte| matches!(byte, b'n' | b'f'))
                && line
                    .get(..10)
                    .is_some_and(|prefix| prefix.iter().all(u8::is_ascii_digit))
            {
                line.get(..10)
                    .and_then(|offset| std::str::from_utf8(offset).ok())
                    .and_then(|offset| offset.parse::<usize>().ok())
                    .and_then(|offset| offset.checked_sub(header_offset))
            } else {
                None
            };
            if let Some(offset) = xref_offset {
                let replacement = format!("{offset:010}");
                if let Some(prefix) = line.get_mut(..10) {
                    prefix.copy_from_slice(replacement.as_bytes());
                    changed = true;
                }
            }
        }
        cursor = line_end.saturating_add(1);
    }

    let Some(startxref) = bytes
        .windows(b"startxref".len())
        .rposition(|window| window == b"startxref")
    else {
        return changed;
    };
    let Some(value_start) = bytes
        .get(startxref + b"startxref".len()..)
        .and_then(|suffix| suffix.iter().position(|byte| byte.is_ascii_digit()))
        .map(|offset| startxref + b"startxref".len() + offset)
    else {
        return changed;
    };
    let value_end = value_start
        + bytes
            .get(value_start..)
            .unwrap_or_default()
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
    let Ok(value) = std::str::from_utf8(bytes.get(value_start..value_end).unwrap_or_default())
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or(())
    else {
        return changed;
    };
    let Some(value) = value.checked_sub(header_offset) else {
        return changed;
    };
    let replacement = value.to_string();
    bytes.splice(value_start..value_end, replacement.bytes());
    changed = true;
    changed
}

pub(crate) fn header_is_offset(bytes: &[u8]) -> bool {
    bytes
        .windows(b"%PDF-".len())
        .position(|window| window == b"%PDF-")
        .is_some_and(|offset| offset > 0)
}

/// Keeps the original bytes available to the syntax inspector while making
/// recoverable header defects acceptable to lopdf's strict loader. veraPDF
/// validates these defects as conformance findings and continues inspecting
/// the rest of the document, so rejecting the document before validation
/// would hide the more useful rule failure.
fn repair_header_syntax(bytes: &mut Vec<u8>) -> bool {
    let Some(marker) = bytes
        .windows(b"%PDF-".len())
        .position(|window| window == b"%PDF-")
    else {
        return false;
    };
    if marker > 0 {
        bytes.drain(..marker);
    }

    let Some(line_end) = bytes
        .get(b"%PDF-".len()..)
        .and_then(|bytes| bytes.iter().position(|byte| matches!(byte, b'\r' | b'\n')))
        .map(|offset| offset + b"%PDF-".len())
    else {
        return marker > 0;
    };

    let mut changed = marker > 0;
    if bytes.get(6) == Some(&b'.')
        && bytes.get(7).is_some_and(u8::is_ascii_digit)
        && bytes.get(5).is_some_and(|byte| !byte.is_ascii_digit())
        && let Some(byte) = bytes.get_mut(5)
    {
        *byte = b'1';
        changed = true;
    }
    if line_end > 8
        && bytes.get(8..line_end).is_some_and(|suffix| {
            suffix
                .iter()
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
        })
        && let Some(byte) = bytes.get_mut(8)
    {
        *byte = b'\n';
        changed = true;
    }
    changed
}

fn repair_startxref_whitespace(bytes: &mut [u8]) -> bool {
    let Some((number_start, number_end, offset)) = final_startxref_parts(bytes) else {
        return false;
    };
    let mut target = offset;
    while target < bytes.len()
        && bytes
            .get(target)
            .is_some_and(|byte| is_pdf_whitespace(*byte))
    {
        target += 1;
    }
    if target == offset || bytes.get(target..target + b"xref".len()) != Some(b"xref") {
        return false;
    }
    let replacement = target.to_string();
    if replacement.len() != number_end - number_start {
        return false;
    }
    let Some(target) = bytes.get_mut(number_start..number_end) else {
        return false;
    };
    target.copy_from_slice(replacement.as_bytes());
    true
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
            let Some(byte) = bytes.get_mut(after_keyword) else {
                return changed;
            };
            *byte = b' ';
            changed = true;
        }
        let mut line_start = after_keyword;
        while line_start < bytes.len() && bytes.get(line_start) != Some(&b'\n') {
            line_start += 1;
        }
        line_start += usize::from(line_start < bytes.len());
        loop {
            let leading = bytes
                .get(line_start..)
                .unwrap_or_default()
                .iter()
                .take_while(|byte| byte.is_ascii_whitespace())
                .count();
            if leading > 0
                && bytes
                    .get(line_start + leading)
                    .is_some_and(u8::is_ascii_digit)
            {
                for byte in bytes
                    .get_mut(line_start..line_start + leading)
                    .unwrap_or_default()
                {
                    *byte = b'0';
                }
                changed = true;
            }
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
            let Some(remainder) = line.get(first_end..) else {
                return changed;
            };
            let Some(second_start) = remainder
                .iter()
                .position(|byte| byte.is_ascii_digit())
                .map(|offset| first_end + offset)
            else {
                return changed;
            };
            let absolute = line_start + first_end;
            let separator_length = second_start - first_end;
            if separator_length > 0 && line.get(first_end..second_start) != Some(b" ") {
                let Some(replacement) = bytes.get_mut(absolute..absolute + separator_length - 1)
                else {
                    return changed;
                };
                for byte in replacement {
                    *byte = b'0';
                }
                let Some(byte) = bytes.get_mut(absolute + separator_length - 1) else {
                    return changed;
                };
                *byte = b' ';
                changed = true;
            }
            let Some((count, _)) = bytes
                .get(line_start..next.saturating_sub(1))
                .and_then(parse_subsection_header)
            else {
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
        let Some(byte) = bytes.get(cursor).copied() else {
            break;
        };
        match byte {
            b'%' if literal_depth == 0 => {
                while cursor < bytes.len()
                    && !matches!(bytes.get(cursor).copied(), Some(b'\r' | b'\n'))
                {
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
                while cursor < bytes.len() && bytes.get(cursor) != Some(&b'>') {
                    let Some(byte) = bytes.get(cursor).copied() else {
                        break;
                    };
                    if !is_pdf_whitespace(byte) && hex(byte).is_none() {
                        let Some(replacement) = bytes.get_mut(cursor) else {
                            return changed;
                        };
                        *replacement = b'0';
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
    let (_, _, offset) = final_startxref_parts(bytes)?;
    let mut target = offset;
    while target < bytes.len()
        && bytes
            .get(target)
            .is_some_and(|byte| is_pdf_whitespace(*byte))
    {
        target += 1;
    }
    Some(target)
}

fn final_startxref_parts(bytes: &[u8]) -> Option<(usize, usize, usize)> {
    let eof = bytes
        .windows(b"%%EOF".len())
        .rposition(|window| window == b"%%EOF")?;
    let start = bytes
        .get(..eof)?
        .windows(b"startxref".len())
        .rposition(|window| window == b"startxref")?
        + b"startxref".len();
    let mut cursor = start;
    while bytes.get(cursor).copied().is_some_and(is_pdf_whitespace) {
        cursor += 1;
    }
    let end = bytes
        .get(cursor..)?
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .map(|length| cursor + length)
        .unwrap_or(bytes.len());
    Some((
        cursor,
        end,
        std::str::from_utf8(bytes.get(cursor..end)?)
            .ok()?
            .parse()
            .ok()?,
    ))
}

fn read_line(bytes: &[u8], start: usize) -> Option<(&[u8], usize)> {
    let end = bytes
        .get(start..)?
        .iter()
        .position(|byte| matches!(byte, b'\r' | b'\n'))
        .map(|offset| start + offset)?;
    Some((bytes.get(start..end)?, single_eol_end(bytes, end)?))
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
    fn header_accepts_a_single_digit_pdf_version() {
        let revisions = [];
        for version in 0..=7 {
            let mut bytes = format!("%PDF-1.{version}\n%").into_bytes();
            bytes.extend_from_slice(&[128, 129, 130, 131, b'\n']);
            assert!(inspect_header(&bytes, &revisions).has_valid_header);
        }
        for header in [
            b"%PDF-1.\n".as_slice(),
            b"%PDF-2.0 extra\n",
            b"%PDF-1.7 extra\n",
        ] {
            assert!(
                !inspect_header(header, &revisions).has_valid_header,
                "{header:?}"
            );
        }
        for header in [b"%PDF-1.8\n".as_slice(), b"%PDF-2.0\n"] {
            assert!(inspect_header(header, &revisions).has_valid_header);
        }
    }

    #[test]
    fn repairs_cross_reference_offsets_after_a_leading_header_offset() {
        let input =
            b" %PDF-1.4\nxref\n0 1\n0000000001 00000 f\ntrailer\n<<>>\nstartxref\n10\n%%EOF";
        let repaired = repair_for_lopdf(input).expect("header offset is repairable");

        assert!(repaired.starts_with(b"%PDF-1.4\n"));
        assert!(
            repaired
                .windows(18)
                .any(|line| line == b"0000000000 00000 f")
        );
        assert!(repaired.ends_with(b"startxref\n9\n%%EOF"));
    }

    #[test]
    fn ignores_indirect_object_headers_inside_stream_data_when_repairing_offsets() {
        let mut bytes = b"%PDF-1.4\n".to_vec();
        let first_object_offset = bytes.len();
        bytes.extend_from_slice(b"1 0 obj\n<< /Length 8 >>\nstream\n2 0 obj\nendstream\nendobj\n");
        let second_object_offset = bytes.len();
        bytes.extend_from_slice(b"2 0 obj\n<<>>\nendobj\nxref\n1 0\n");
        bytes.extend_from_slice(b"0000000000 00000 n \n2 0\n0000000000 00000 n \n");
        bytes.extend_from_slice(b"trailer\n<<>>\n");

        assert_eq!(
            indirect_object_header(&bytes, first_object_offset)
                .map(|(object, generation, _)| (object, generation)),
            Some((1, 0))
        );
        let (_, _, after_header) = indirect_object_header(&bytes, first_object_offset).unwrap();
        assert_eq!(
            stream_end_after_object(&bytes, after_header),
            Some(
                first_object_offset + b"1 0 obj\n<< /Length 8 >>\nstream\n2 0 obj\nendstream".len()
            )
        );
        assert!(repair_xref_offsets(&mut bytes));
        assert!(
            bytes
                .windows(19)
                .any(|line| { line == format!("{first_object_offset:010} 00000 n ").as_bytes() })
        );
        assert!(
            bytes
                .windows(19)
                .any(|line| { line == format!("{second_object_offset:010} 00000 n ").as_bytes() })
        );
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
    fn revision_inspection_preserves_the_configured_revision_limit() {
        let first = b"xref\n0 0\ntrailer\n<<>>\n";
        let second_offset = first.len();
        let bytes = [
            first.as_slice(),
            format!("xref\n0 0\ntrailer\n<< /Prev 0 >>\nstartxref\n{second_offset}\n%%EOF\n")
                .as_bytes(),
        ]
        .concat();
        let limits = SafetyLimits {
            max_xref_revisions: 1,
            ..SafetyLimits::default()
        };

        assert!(matches!(
            inspect_revisions(&bytes, &limits),
            Err(PdfError::ReferenceDepth(1))
        ));
    }

    #[test]
    fn preflight_rejects_many_indirect_objects_before_loading_them() {
        use std::io::Write as _;

        let object_count = 32;
        let mut bytes = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::with_capacity(object_count);
        for object_number in 1..=object_count {
            offsets.push(bytes.len());
            writeln!(bytes, "{object_number} 0 obj\nnull\nendobj").expect("write object");
        }
        let xref_offset = bytes.len();
        writeln!(bytes, "xref\n0 {}", object_count + 1).expect("write xref header");
        bytes.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets {
            writeln!(bytes, "{offset:010} 00000 n ").expect("write xref entry");
        }
        writeln!(bytes, "trailer\n<< /Size {} >>", object_count + 1).expect("write trailer");
        writeln!(bytes, "startxref\n{xref_offset}\n%%EOF").expect("write footer");

        let limits = SafetyLimits {
            max_object_count: 8,
            ..SafetyLimits::default()
        };
        assert!(matches!(
            preflight_object_limit(&bytes, &limits),
            Err(PdfError::TooManyObjects {
                actual: 32,
                limit: 8
            })
        ));
    }

    #[test]
    fn permits_a_blank_line_between_xref_entries_and_the_trailer() {
        let bytes = b"xref\n0 1\n0000000000 65535 f \n\ntrailer\n<<>>\nstartxref\n0\n%%EOF\n";
        let revisions = inspect_revisions(bytes, &SafetyLimits::default()).unwrap();
        let [revision] = revisions.as_slice() else {
            panic!("expected one xref revision");
        };
        assert!(revision.eol_compliant);
    }
}
