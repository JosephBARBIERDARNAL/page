//! Shared bounds and preflight checks for parsing untrusted XML payloads (XMP metadata, XFA
//! forms) with `roxmltree`, whose own node limit does not by itself bound recursion depth during
//! parsing.

use roxmltree::{Document, ParsingOptions};

/// `roxmltree`'s `nodes_limit` bounds total node count but not nesting depth; a payload can stay
/// under that limit while nesting deep enough to pressure the parser's recursion.
pub(crate) const MAX_BOUNDED_XML_NODES: u32 = 100_000;
pub(crate) const MAX_BOUNDED_XML_DEPTH: usize = 128;

pub(crate) fn bounded_parsing_options<'a>(max_nodes: u32) -> ParsingOptions<'a> {
    ParsingOptions {
        allow_dtd: false,
        nodes_limit: max_nodes,
        ..ParsingOptions::default()
    }
}

/// Scans raw XML for element nesting deeper than `max_depth` without building a tree, so a
/// deeply nested payload is rejected before `roxmltree` parses it.
pub(crate) fn preflight_xml_depth(xml: &str, max_depth: usize) -> Result<(), String> {
    let bytes = xml.as_bytes();
    let mut position = 0_usize;
    let mut depth = 0_usize;
    while let Some(relative) = bytes
        .get(position..)
        .and_then(|tail| tail.iter().position(|byte| *byte == b'<'))
    {
        position += relative;
        let Some(tail) = bytes.get(position..) else {
            break;
        };
        if tail.starts_with(b"<!--") {
            position = find_xml_delimiter(bytes, position + 4, b"-->")?;
            continue;
        }
        if tail.starts_with(b"<![CDATA[") {
            position = find_xml_delimiter(bytes, position + 9, b"]]>")?;
            continue;
        }
        if tail.starts_with(b"<?") {
            position = find_xml_delimiter(bytes, position + 2, b"?>")?;
            continue;
        }
        if tail.starts_with(b"<!") {
            return Err(
                "DTD and XML declarations other than comments or CDATA are disabled".to_owned(),
            );
        }
        let (end, self_closing) = find_xml_tag_end(bytes, position + 1)?;
        if tail.starts_with(b"</") {
            depth = depth.saturating_sub(1);
        } else if !self_closing {
            depth += 1;
            if depth > max_depth {
                return Err(format!("XML nesting depth exceeds {max_depth}"));
            }
        }
        position = end;
    }
    Ok(())
}

pub(crate) fn find_xml_delimiter(
    bytes: &[u8],
    start: usize,
    delimiter: &[u8],
) -> Result<usize, String> {
    bytes
        .get(start..)
        .unwrap_or_default()
        .windows(delimiter.len())
        .position(|window| window == delimiter)
        .map(|relative| start + relative + delimiter.len())
        .ok_or_else(|| "unterminated XML construct".to_owned())
}

pub(crate) fn find_xml_tag_end(bytes: &[u8], start: usize) -> Result<(usize, bool), String> {
    let mut quote = None;
    let mut position = start;
    while let Some(byte) = bytes.get(position).copied() {
        match (quote, byte) {
            (Some(expected), actual) if expected == actual => quote = None,
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b'>') => {
                let self_closing = bytes
                    .get(start..position)
                    .unwrap_or_default()
                    .iter()
                    .rev()
                    .find(|byte| !byte.is_ascii_whitespace())
                    == Some(&b'/');
                return Ok((position + 1, self_closing));
            }
            _ => {}
        }
        position += 1;
    }
    Err("unterminated XML tag".to_owned())
}

/// Re-checks nesting depth on the parsed tree, catching constructs (such as CDATA-escaped
/// content) that the byte-level preflight scan does not expand.
pub(crate) fn validate_xml_depth(document: &Document<'_>, max_depth: usize) -> Result<(), String> {
    let mut stack = vec![(document.root(), 0_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > max_depth {
            return Err(format!("XML nesting depth exceeds {max_depth}"));
        }
        stack.extend(
            node.children()
                .map(|child| (child, depth + usize::from(child.is_element()))),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_rejects_deep_nesting() {
        let mut xml = String::new();
        for _ in 0..5 {
            xml.push_str("<a>");
        }
        xml.push_str("<a/>");
        for _ in 0..5 {
            xml.push_str("</a>");
        }
        assert!(preflight_xml_depth(&xml, 3).is_err());
        preflight_xml_depth(&xml, 10).unwrap();
    }

    #[test]
    fn bounded_parsing_options_rejects_dtd() {
        let options = bounded_parsing_options(10);
        Document::parse_with_options("<!DOCTYPE x><x/>", options).unwrap_err();
    }
}
