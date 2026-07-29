use lopdf::{Document, Object};

use crate::content_support::is_pdf_boundary;
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::resolve_optional;

#[derive(Clone, Debug, Default)]
pub(crate) struct StreamSafetySummary {
    pub(crate) external_stream_entries: Vec<StreamFailure>,
    pub(crate) lzw_filters: Vec<PdfObjectId>,
    pub(crate) xref_streams: Vec<PdfObjectId>,
    pub(crate) invalid_lengths: Vec<PdfObjectId>,
    pub(crate) invalid_eol_markers: Vec<PdfObjectId>,
    pub(crate) has_odd_hex_string: bool,
    pub(crate) has_non_hex_character: bool,
    pub(crate) has_invalid_xref_subsection_spacing: bool,
    pub(crate) has_invalid_xref_eol: bool,
    pub(crate) has_invalid_indirect_object_syntax: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct StreamFailure {
    pub(crate) object_id: PdfObjectId,
    pub(crate) keys: Vec<&'static str>,
}

pub(crate) fn inspect(
    document: &Document,
    limits: &SafetyLimits,
    bytes: &[u8],
) -> Result<StreamSafetySummary, PdfError> {
    let mut summary = StreamSafetySummary::default();
    let mut stream_data_ranges = Vec::new();
    let mut used_stream_starts = Vec::new();
    let mut all_stream_ranges_known = true;
    for (object_id, object) in &document.objects {
        let Object::Stream(stream) = object else {
            continue;
        };
        if stream.dict.get_type().ok() == Some(b"XRef".as_slice()) {
            summary.xref_streams.push((*object_id).into());
        }
        let has_parser_position = stream.start_position.is_some();
        let raw_start = stream
            .start_position
            .or_else(|| locate_raw_stream_data_start(bytes, &stream.content, &used_stream_starts));
        if let Some(start) = raw_start {
            used_stream_starts.push(start);
            inspect_raw_stream_syntax(
                document,
                stream,
                *object_id,
                start,
                limits,
                bytes,
                &mut summary,
            )?;
        }
        let raw_range = match raw_start {
            Some(start) => raw_stream_data_range(document, stream, start, limits)?,
            None => None,
        };
        if let Some(range) = raw_range.filter(|_| has_parser_position) {
            stream_data_ranges.push(range);
        } else {
            all_stream_ranges_known = false;
        }
        let keys = [
            (b"F".as_slice(), "F"),
            (b"FFilter".as_slice(), "FFilter"),
            (b"FDecodeParms".as_slice(), "FDecodeParms"),
        ]
        .into_iter()
        .filter_map(|(key, name)| {
            stream
                .dict
                .get(key)
                .ok()
                .filter(|value| !matches!(value, Object::Null))
                .map(|_| name)
        })
        .collect::<Vec<_>>();
        if !keys.is_empty() {
            summary.external_stream_entries.push(StreamFailure {
                object_id: (*object_id).into(),
                keys,
            });
        }
        if stream
            .dict
            .get(b"Filter")
            .ok()
            .map(|value| filter_contains_lzw_decode(document, value, limits.max_reference_depth))
            .transpose()?
            .unwrap_or(false)
        {
            summary.lzw_filters.push((*object_id).into());
        }
    }
    if all_stream_ranges_known {
        inspect_hex_strings(bytes, &stream_data_ranges, &mut summary);
        inspect_xref_syntax(bytes, &stream_data_ranges, &mut summary);
        inspect_indirect_object_syntax(bytes, &stream_data_ranges, &mut summary);
    }
    Ok(summary)
}

fn inspect_raw_stream_syntax(
    document: &Document,
    stream: &lopdf::Stream,
    object_id: lopdf::ObjectId,
    start: usize,
    limits: &SafetyLimits,
    bytes: &[u8],
    summary: &mut StreamSafetySummary,
) -> Result<(), PdfError> {
    let declared_length = stream
        .dict
        .get(b"Length")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_i64().ok());
    let boundary = declared_length
        .and_then(|length| usize::try_from(length).ok())
        .and_then(|length| start.checked_add(length))
        .and_then(|data_end| {
            endstream_after_eol(bytes, data_end).map(|endstream| (data_end, endstream))
        });
    let valid_start = stream_keyword_has_required_eol(bytes, start);
    let valid_end = boundary.is_some();
    if !valid_start || !valid_end {
        summary.invalid_eol_markers.push(object_id.into());
    }
    if boundary.is_none() {
        summary.invalid_lengths.push(object_id.into());
    }
    Ok(())
}

fn stream_keyword_has_required_eol(bytes: &[u8], start: usize) -> bool {
    let eol_start = match (
        bytes.get(start.wrapping_sub(2)),
        bytes.get(start.wrapping_sub(1)),
    ) {
        (Some(b'\r'), Some(b'\n')) => start - 2,
        (_, Some(b'\n')) => start - 1,
        _ => return false,
    };
    bytes
        .get(..eol_start)
        .is_some_and(|before_eol| before_eol.ends_with(b"stream"))
}

fn endstream_after_eol(bytes: &[u8], data_end: usize) -> Option<usize> {
    let eol_end = match (bytes.get(data_end), bytes.get(data_end + 1)) {
        (Some(b'\r'), Some(b'\n')) => data_end + 2,
        (Some(b'\r' | b'\n'), _) => data_end + 1,
        _ => return None,
    };
    (bytes.get(eol_end..eol_end + b"endstream".len()) == Some(b"endstream")).then_some(eol_end)
}

fn raw_stream_data_range(
    document: &Document,
    stream: &lopdf::Stream,
    start: usize,
    limits: &SafetyLimits,
) -> Result<Option<std::ops::Range<usize>>, PdfError> {
    let Some(length) = stream
        .dict
        .get(b"Length")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_i64().ok())
        .and_then(|length| usize::try_from(length).ok())
    else {
        return Ok(None);
    };
    Ok(start.checked_add(length).map(|end| start..end))
}

fn locate_raw_stream_data_start(
    bytes: &[u8],
    content: &[u8],
    used_starts: &[usize],
) -> Option<usize> {
    bytes
        .windows(b"stream".len())
        .enumerate()
        .find_map(|(offset, window)| {
            (window == b"stream"
                && is_pdf_boundary(bytes.get(offset.wrapping_sub(1)).copied())
                && is_pdf_boundary(bytes.get(offset + b"stream".len()).copied()))
            .then(|| stream_data_start_after_keyword(bytes, offset + b"stream".len()))?
            .filter(|start| !used_starts.contains(start))
            .filter(|start| bytes.get(*start..start.saturating_add(content.len())) == Some(content))
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

fn inspect_hex_strings(
    bytes: &[u8],
    stream_data_ranges: &[std::ops::Range<usize>],
    summary: &mut StreamSafetySummary,
) {
    let mut cursor = 0;
    while cursor < bytes.len() {
        if let Some(range) = stream_data_ranges
            .iter()
            .find(|range| range.contains(&cursor))
        {
            cursor = range.end;
            continue;
        }
        match bytes[cursor] {
            b'%' => {
                cursor += 1;
                while cursor < bytes.len() && !matches!(bytes[cursor], b'\r' | b'\n') {
                    cursor += 1;
                }
            }
            b'(' => cursor = skip_literal_string(bytes, cursor + 1),
            b'<' if bytes.get(cursor + 1) == Some(&b'<') => cursor += 2,
            b'<' => {
                cursor += 1;
                let mut count = 0;
                while cursor < bytes.len() && bytes[cursor] != b'>' {
                    if !bytes[cursor].is_ascii_whitespace() {
                        count += 1;
                        summary.has_non_hex_character |= !bytes[cursor].is_ascii_hexdigit();
                    }
                    cursor += 1;
                }
                summary.has_odd_hex_string |= count % 2 != 0;
                cursor += usize::from(cursor < bytes.len());
            }
            _ => cursor += 1,
        }
    }
}

fn skip_literal_string(bytes: &[u8], mut cursor: usize) -> usize {
    let mut nesting = 1;
    while cursor < bytes.len() && nesting > 0 {
        match bytes[cursor] {
            b'\\' => cursor = cursor.saturating_add(2),
            b'(' => {
                nesting += 1;
                cursor += 1;
            }
            b')' => {
                nesting -= 1;
                cursor += 1;
            }
            _ => cursor += 1,
        }
    }
    cursor
}

fn inspect_xref_syntax(
    bytes: &[u8],
    stream_data_ranges: &[std::ops::Range<usize>],
    summary: &mut StreamSafetySummary,
) {
    let mut cursor = 0;
    while cursor < bytes.len() {
        if let Some(range) = stream_data_ranges
            .iter()
            .find(|range| range.contains(&cursor))
        {
            cursor = range.end;
            continue;
        }
        match bytes[cursor] {
            b'%' => {
                cursor += 1;
                while cursor < bytes.len() && !matches!(bytes[cursor], b'\r' | b'\n') {
                    cursor += 1;
                }
            }
            b'(' => cursor = skip_literal_string(bytes, cursor + 1),
            b'<' if bytes.get(cursor + 1) == Some(&b'<') => cursor += 2,
            b'<' => {
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor] != b'>' {
                    cursor += 1;
                }
                cursor += usize::from(cursor < bytes.len());
            }
            b'x' if bytes.get(cursor..cursor + 4) == Some(b"xref")
                && is_pdf_boundary(bytes.get(cursor.wrapping_sub(1)).copied())
                && is_pdf_boundary(bytes.get(cursor + 4).copied()) =>
            {
                inspect_xref_table(bytes, cursor + 4, summary);
                cursor += 4;
            }
            _ => cursor += 1,
        }
    }
}

fn inspect_xref_table(bytes: &[u8], after_xref: usize, summary: &mut StreamSafetySummary) {
    let Some(mut cursor) = single_eol_end(bytes, after_xref) else {
        summary.has_invalid_xref_eol = true;
        return;
    };
    loop {
        let Some((line, next)) = read_line(bytes, cursor) else {
            return;
        };
        if line == b"trailer" {
            return;
        }
        let Some(entry_count) = subsection_entry_count(line) else {
            summary.has_invalid_xref_subsection_spacing = true;
            return;
        };
        cursor = next;
        for _ in 0..entry_count {
            let Some((_, next)) = read_line(bytes, cursor) else {
                return;
            };
            cursor = next;
        }
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

fn read_line(bytes: &[u8], start: usize) -> Option<(&[u8], usize)> {
    let end = bytes[start..]
        .iter()
        .position(|byte| matches!(byte, b'\r' | b'\n'))
        .map(|offset| start + offset)?;
    Some((&bytes[start..end], single_eol_end(bytes, end)?))
}

fn subsection_entry_count(line: &[u8]) -> Option<usize> {
    let separator = line.iter().position(|byte| !byte.is_ascii_digit())?;
    (separator > 0 && line.get(separator) == Some(&b' ') && line.get(separator + 1) != Some(&b' '))
        .then_some(())?;
    let count_end = separator
        + 1
        + line[separator + 1..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
    (count_end == line.len()).then_some(())?;
    std::str::from_utf8(&line[separator + 1..count_end])
        .ok()?
        .parse()
        .ok()
}

fn inspect_indirect_object_syntax(
    bytes: &[u8],
    stream_data_ranges: &[std::ops::Range<usize>],
    summary: &mut StreamSafetySummary,
) {
    let mut cursor = 0;
    while cursor < bytes.len() {
        if let Some(range) = stream_data_ranges
            .iter()
            .find(|range| range.contains(&cursor))
        {
            cursor = range.end;
            continue;
        }
        match bytes[cursor] {
            b'%' => {
                cursor += 1;
                while cursor < bytes.len() && !matches!(bytes[cursor], b'\r' | b'\n') {
                    cursor += 1;
                }
            }
            b'(' => cursor = skip_literal_string(bytes, cursor + 1),
            b'<' if bytes.get(cursor + 1) == Some(&b'<') => cursor += 2,
            b'<' => {
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor] != b'>' {
                    cursor += 1;
                }
                cursor += usize::from(cursor < bytes.len());
            }
            byte if byte.is_ascii_digit()
                && is_pdf_boundary(bytes.get(cursor.wrapping_sub(1)).copied()) =>
            {
                if let Some((after_obj, valid)) = indirect_object_header_end(bytes, cursor) {
                    summary.has_invalid_indirect_object_syntax |= !valid;
                    cursor = after_obj;
                } else {
                    cursor += 1;
                }
            }
            b'e' if bytes.get(cursor..cursor + b"endobj".len()) == Some(b"endobj")
                && is_pdf_boundary(bytes.get(cursor.wrapping_sub(1)).copied())
                && is_pdf_boundary(bytes.get(cursor + b"endobj".len()).copied()) =>
            {
                summary.has_invalid_indirect_object_syntax |= !is_eol_before(bytes, cursor)
                    || single_eol_end(bytes, cursor + b"endobj".len()).is_none();
                cursor += b"endobj".len();
            }
            _ => cursor += 1,
        }
    }
}

fn indirect_object_header_end(bytes: &[u8], start: usize) -> Option<(usize, bool)> {
    let object_number_end = skip_digits(bytes, start);
    let (generation_start, first_separator_length) = skip_whitespace(bytes, object_number_end);
    (generation_start > object_number_end).then_some(())?;
    let generation_end = skip_digits(bytes, generation_start);
    (generation_end > generation_start).then_some(())?;
    let (obj_start, second_separator_length) = skip_whitespace(bytes, generation_end);
    (obj_start > generation_end).then_some(())?;
    (bytes.get(obj_start..obj_start + b"obj".len()) == Some(b"obj")
        && is_pdf_boundary(bytes.get(obj_start + b"obj".len()).copied()))
    .then_some(())?;
    let after_obj = obj_start + b"obj".len();
    Some((
        after_obj,
        first_separator_length == 1
            && second_separator_length == 1
            && is_eol_before(bytes, start)
            && single_eol_end(bytes, after_obj).is_some(),
    ))
}

fn skip_digits(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        cursor += 1;
    }
    cursor
}

fn skip_whitespace(bytes: &[u8], mut cursor: usize) -> (usize, usize) {
    let start = cursor;
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    (cursor, cursor - start)
}

fn is_eol_before(bytes: &[u8], cursor: usize) -> bool {
    matches!(bytes.get(cursor.wrapping_sub(1)), Some(b'\r' | b'\n'))
}

fn filter_contains_lzw_decode(
    document: &Document,
    filter: &Object,
    maximum_depth: usize,
) -> Result<bool, PdfError> {
    let Some(filter) = resolve_optional(document, filter, maximum_depth)? else {
        return Ok(false);
    };
    Ok(match filter {
        Object::Name(name) => name.as_slice() == b"LZWDecode",
        Object::Array(filters) => filters.iter().try_fold(false, |found, filter| {
            Ok::<bool, PdfError>(
                found || filter_contains_lzw_decode(document, filter, maximum_depth)?,
            )
        })?,
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use lopdf::{Dictionary, Document, Stream};

    use super::{StreamSafetySummary, inspect_raw_stream_syntax};
    use crate::SafetyLimits;

    #[test]
    fn checks_raw_stream_length_and_eol_markers() {
        let document = Document::with_version("1.4");
        let mut stream = Stream::new(Dictionary::new(), b"abc".to_vec());
        stream.start_position = Some(7);
        let bytes = b"stream\nabc\nendstream";
        let mut summary = StreamSafetySummary::default();
        inspect_raw_stream_syntax(
            &document,
            &stream,
            (1, 0),
            7,
            &SafetyLimits::default(),
            bytes,
            &mut summary,
        )
        .expect("inspect valid stream");
        assert!(summary.invalid_lengths.is_empty());
        assert!(summary.invalid_eol_markers.is_empty());

        stream.dict.set("Length", 2);
        let mut summary = StreamSafetySummary::default();
        inspect_raw_stream_syntax(
            &document,
            &stream,
            (1, 0),
            7,
            &SafetyLimits::default(),
            bytes,
            &mut summary,
        )
        .expect("inspect mismatched stream");
        assert_eq!(summary.invalid_lengths.len(), 1);

        let bytes = b"stream abc\nendstream";
        stream.start_position = Some(7);
        let mut summary = StreamSafetySummary::default();
        inspect_raw_stream_syntax(
            &document,
            &stream,
            (1, 0),
            7,
            &SafetyLimits::default(),
            bytes,
            &mut summary,
        )
        .expect("inspect invalid EOL stream");
        assert_eq!(summary.invalid_eol_markers.len(), 1);

        stream.dict.set("Length", 17);
        stream.start_position = Some(7);
        let bytes = b"stream\nabc\nendstream\nxyz\nendstream";
        let mut summary = StreamSafetySummary::default();
        inspect_raw_stream_syntax(
            &document,
            &stream,
            (1, 0),
            7,
            &SafetyLimits::default(),
            bytes,
            &mut summary,
        )
        .expect("inspect stream data containing endstream text");
        assert!(summary.invalid_lengths.is_empty());
        assert!(summary.invalid_eol_markers.is_empty());

        stream.dict.set("Length", 3);
        let bytes = b"stream \nabc\nendstream";
        let mut summary = StreamSafetySummary::default();
        inspect_raw_stream_syntax(
            &document,
            &stream,
            (1, 0),
            7,
            &SafetyLimits::default(),
            bytes,
            &mut summary,
        )
        .expect("inspect stream with extra keyword space");
        assert_eq!(summary.invalid_eol_markers.len(), 1);
    }

    #[test]
    fn inspects_direct_length_streams_without_parser_offsets() {
        let mut source = Document::with_version("1.4");
        source.add_object(Stream::new(Dictionary::new(), b"abc".to_vec()));
        let mut bytes = Vec::new();
        source.save_to(&mut bytes).expect("serialize stream");
        let document = Document::load_mem(&bytes).expect("parse stream");
        assert!(
            document
                .objects
                .values()
                .find_map(|object| object.as_stream().ok())
                .is_some_and(|stream| stream.start_position.is_none())
        );

        let summary =
            super::inspect(&document, &SafetyLimits::default(), &bytes).expect("inspect stream");
        assert!(summary.invalid_lengths.is_empty());
        assert!(summary.invalid_eol_markers.is_empty());
    }

    #[test]
    fn checks_hex_string_syntax_without_scanning_comments_or_stream_data() {
        let mut summary = StreamSafetySummary::default();
        let stream_range = 41..54;
        super::inspect_hex_strings(
            b"<A0> <A 0> (literal <F>) % comment <F>\nstream\n<F>\nendstream",
            std::slice::from_ref(&stream_range),
            &mut summary,
        );
        assert!(!summary.has_odd_hex_string);
        assert!(!summary.has_non_hex_character);

        super::inspect_hex_strings(b"<A> <G0>", &[], &mut summary);
        assert!(summary.has_odd_hex_string);
        assert!(summary.has_non_hex_character);
    }

    #[test]
    fn checks_xref_eol_and_subsection_header_syntax() {
        let mut summary = StreamSafetySummary::default();
        super::inspect_xref_syntax(
            b"xref\n0 2\n0000000000 65535 f \n0000000010 00000 n \ntrailer\n",
            &[],
            &mut summary,
        );
        assert!(!summary.has_invalid_xref_eol);
        assert!(!summary.has_invalid_xref_subsection_spacing);

        super::inspect_xref_syntax(
            b"xref\n0  1\n0000000000 65535 f \ntrailer\n",
            &[],
            &mut summary,
        );
        assert!(summary.has_invalid_xref_subsection_spacing);

        let mut summary = StreamSafetySummary::default();
        super::inspect_xref_syntax(
            b"xref 0 1\n0000000000 65535 f \ntrailer\n",
            &[],
            &mut summary,
        );
        assert!(summary.has_invalid_xref_eol);
    }

    #[test]
    fn checks_indirect_object_syntax_without_scanning_strings_or_stream_data() {
        let mut summary = StreamSafetySummary::default();
        super::inspect_indirect_object_syntax(
            b"\n1 0 obj\n<< /Value (2 0 obj) >>\nendobj\n",
            &[],
            &mut summary,
        );
        assert!(!summary.has_invalid_indirect_object_syntax);

        super::inspect_indirect_object_syntax(b"2  0 obj\nnull\nendobj\n", &[], &mut summary);
        assert!(summary.has_invalid_indirect_object_syntax);

        let mut summary = StreamSafetySummary::default();
        super::inspect_indirect_object_syntax(b"3 0 obj null\nendobj\n", &[], &mut summary);
        assert!(summary.has_invalid_indirect_object_syntax);
    }
}
