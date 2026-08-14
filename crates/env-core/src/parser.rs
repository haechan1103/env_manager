use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::{EnvError, EnvErrorCode, EnvResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NewlineStyle {
    Lf,
    CrLf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Blank { span: Span },
    Comment { span: Span, content: Span },
    GroupDirective { span: Span, name: Span },
    Assignment { span: Span, key: Span, value: Span },
    Opaque { span: Span },
}

impl Node {
    pub const fn span(&self) -> Span {
        match self {
            Self::Blank { span }
            | Self::Comment { span, .. }
            | Self::GroupDirective { span, .. }
            | Self::Assignment { span, .. }
            | Self::Opaque { span } => *span,
        }
    }
}

pub struct Document {
    source: Vec<u8>,
    nodes: Vec<Node>,
    newline_style: NewlineStyle,
    has_bom: bool,
    has_final_newline: bool,
}

impl Drop for Document {
    fn drop(&mut self) {
        self.source.zeroize();
    }
}

pub struct AssignmentRef<'a> {
    pub node_index: usize,
    pub key: &'a str,
    value: &'a [u8],
    pub span: Span,
    pub value_span: Span,
}

impl AssignmentRef<'_> {
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub(crate) fn value_bytes(&self) -> &[u8] {
        self.value
    }
}

impl Document {
    pub fn parse(bytes: Vec<u8>, path: &Path) -> EnvResult<Self> {
        if bytes.contains(&0) {
            return Err(EnvError::new(
                EnvErrorCode::ParseUnsupported,
                format!(
                    "NUL 바이트가 포함된 파일은 지원하지 않습니다: {}",
                    path.display()
                ),
            ));
        }
        std::str::from_utf8(&bytes).map_err(|_| EnvError::unsupported_encoding(path))?;

        let has_bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
        let content_start = usize::from(has_bom) * 3;
        let newline_style = if bytes.windows(2).any(|pair| pair == b"\r\n") {
            NewlineStyle::CrLf
        } else {
            NewlineStyle::Lf
        };
        let has_final_newline = bytes.ends_with(b"\n");
        let lines = line_spans(&bytes, content_start);
        let mut nodes = Vec::new();
        let mut line_index = 0;

        while line_index < lines.len() {
            let line_span = lines[line_index];
            let body_end = line_body_end(&bytes, line_span);
            let body = &bytes[line_span.start..body_end];

            if body.iter().all(u8::is_ascii_whitespace) {
                nodes.push(Node::Blank { span: line_span });
                line_index += 1;
                continue;
            }

            let trimmed_start = body
                .iter()
                .position(|byte| !byte.is_ascii_whitespace())
                .unwrap_or(body.len());
            if body.get(trimmed_start) == Some(&b'#') {
                if let Some((name_start, name_end)) = parse_group_directive(body, trimmed_start) {
                    nodes.push(Node::GroupDirective {
                        span: line_span,
                        name: Span::new(line_span.start + name_start, line_span.start + name_end),
                    });
                } else {
                    let content_start = trimmed_start + 1;
                    nodes.push(Node::Comment {
                        span: line_span,
                        content: Span::new(line_span.start + content_start, body_end),
                    });
                }
                line_index += 1;
                continue;
            }

            if let Some(parsed) = parse_assignment_start(body, line_span.start) {
                let mut full_end = line_span.end;
                let mut value_end = parsed.value_end;
                let mut consumed_line = line_index;

                if let Some(quote) = parsed.open_quote {
                    let mut cursor = parsed.value_start + 1;
                    let mut escaped = false;
                    let mut closing = None;
                    loop {
                        while cursor < bytes.len() {
                            let byte = bytes[cursor];
                            if quote == b'"' && byte == b'\\' && !escaped {
                                escaped = true;
                                cursor += 1;
                                continue;
                            }
                            if byte == quote && !escaped {
                                closing = Some(cursor + 1);
                                break;
                            }
                            escaped = false;
                            cursor += 1;
                        }
                        if closing.is_some() || consumed_line + 1 >= lines.len() {
                            break;
                        }
                        consumed_line += 1;
                        cursor = lines[consumed_line].start;
                        full_end = lines[consumed_line].end;
                    }
                    if let Some(closing) = closing {
                        value_end = closing;
                        full_end = lines[consumed_line].end;
                    } else {
                        nodes.push(Node::Opaque {
                            span: Span::new(line_span.start, full_end),
                        });
                        line_index = consumed_line + 1;
                        continue;
                    }
                }

                nodes.push(Node::Assignment {
                    span: Span::new(line_span.start, full_end),
                    key: parsed.key,
                    value: Span::new(parsed.value_start, value_end),
                });
                line_index = consumed_line + 1;
                continue;
            }

            nodes.push(Node::Opaque { span: line_span });
            line_index += 1;
        }

        Ok(Self {
            source: bytes,
            nodes,
            newline_style,
            has_bom,
            has_final_newline,
        })
    }

    pub fn source(&self) -> &[u8] {
        &self.source
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub const fn newline_style(&self) -> NewlineStyle {
        self.newline_style
    }

    pub const fn has_bom(&self) -> bool {
        self.has_bom
    }

    pub const fn has_final_newline(&self) -> bool {
        self.has_final_newline
    }

    pub fn text(&self, span: Span) -> &str {
        std::str::from_utf8(&self.source[span.range()]).unwrap_or_default()
    }

    pub fn assignments(&self) -> Vec<AssignmentRef<'_>> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(node_index, node)| match node {
                Node::Assignment {
                    span, key, value, ..
                } => Some(AssignmentRef {
                    node_index,
                    key: self.text(*key),
                    value: &self.source[value.range()],
                    span: *span,
                    value_span: *value,
                }),
                _ => None,
            })
            .collect()
    }

    pub fn assignment(&self, key: &str) -> EnvResult<AssignmentRef<'_>> {
        let mut matches = self
            .assignments()
            .into_iter()
            .filter(|assignment| assignment.key == key);
        let found = matches
            .next()
            .ok_or_else(|| EnvError::invalid(format!("{key} 변수를 찾지 못했습니다.")))?;
        if matches.next().is_some() {
            return Err(EnvError::new(
                EnvErrorCode::ParseAmbiguousDuplicateKey,
                format!("{key} 변수가 파일에 여러 번 있습니다."),
            ));
        }
        Ok(found)
    }

    pub fn duplicate_keys(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for assignment in self.assignments() {
            *counts.entry(assignment.key.to_owned()).or_insert(0) += 1;
        }
        counts.retain(|_, count| *count > 1);
        counts
    }

    pub fn replace_value(&self, key: &str, new_value: &str) -> EnvResult<Vec<u8>> {
        let assignment = self.assignment(key)?;
        let encoded = encode_value(new_value, assignment.value_bytes());
        Ok(replace_span(
            &self.source,
            assignment.value_span,
            encoded.as_bytes(),
        ))
    }

    pub fn decoded_value(&self, key: &str) -> EnvResult<String> {
        let assignment = self.assignment(key)?;
        decode_value(assignment.value_bytes())
    }

    pub fn replace_span(&self, span: Span, replacement: &[u8]) -> Vec<u8> {
        replace_span(&self.source, span, replacement)
    }
}

struct ParsedAssignment {
    key: Span,
    value_start: usize,
    value_end: usize,
    open_quote: Option<u8>,
}

fn parse_assignment_start(line: &[u8], absolute_start: usize) -> Option<ParsedAssignment> {
    let mut index = 0;
    skip_spaces(line, &mut index);
    if line.get(index..index + 6) == Some(b"export") {
        let next = line.get(index + 6).copied();
        if next.is_some_and(|byte| byte.is_ascii_whitespace()) {
            index += 6;
            skip_spaces(line, &mut index);
        }
    }

    let key_start = index;
    let first = *line.get(index)?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    index += 1;
    while line
        .get(index)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        index += 1;
    }
    let key_end = index;
    skip_spaces(line, &mut index);
    if line.get(index) != Some(&b'=') {
        return None;
    }
    index += 1;
    skip_spaces(line, &mut index);
    let value_start_in_line = index;
    let open_quote = line
        .get(index)
        .copied()
        .filter(|byte| matches!(byte, b'\'' | b'"'));
    let value_end_in_line = if open_quote.is_some() {
        line.len()
    } else {
        unquoted_value_end(line, index)
    };

    Some(ParsedAssignment {
        key: Span::new(absolute_start + key_start, absolute_start + key_end),
        value_start: absolute_start + value_start_in_line,
        value_end: absolute_start + value_end_in_line,
        open_quote,
    })
}

fn parse_group_directive(line: &[u8], hash_index: usize) -> Option<(usize, usize)> {
    let after_hash = &line[hash_index + 1..];
    let trimmed = trim_ascii_start(after_hash);
    let prefix = b"@group";
    if !trimmed.starts_with(prefix) {
        return None;
    }
    let after_prefix = &trimmed[prefix.len()..];
    if !after_prefix
        .first()
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        return None;
    }
    let name = trim_ascii(after_prefix);
    if name.is_empty() {
        return None;
    }
    let base = line.len() - after_hash.len() + (after_hash.len() - trimmed.len()) + prefix.len();
    let leading = after_prefix.len() - trim_ascii_start(after_prefix).len();
    let name_start = base + leading;
    Some((name_start, name_start + name.len()))
}

fn line_spans(source: &[u8], start: usize) -> Vec<Span> {
    if start >= source.len() {
        return Vec::new();
    }
    let mut spans = Vec::new();
    let mut line_start = start;
    for (offset, byte) in source[start..].iter().enumerate() {
        if *byte == b'\n' {
            let end = start + offset + 1;
            spans.push(Span::new(line_start, end));
            line_start = end;
        }
    }
    if line_start < source.len() {
        spans.push(Span::new(line_start, source.len()));
    }
    spans
}

fn line_body_end(source: &[u8], span: Span) -> usize {
    let mut end = span.end;
    if end > span.start && source[end - 1] == b'\n' {
        end -= 1;
    }
    if end > span.start && source[end - 1] == b'\r' {
        end -= 1;
    }
    end
}

fn skip_spaces(line: &[u8], index: &mut usize) {
    while line
        .get(*index)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        *index += 1;
    }
}

fn unquoted_value_end(line: &[u8], start: usize) -> usize {
    let mut comment_start = line.len();
    for index in start..line.len() {
        if line[index] == b'#'
            && (index == start || line[index.saturating_sub(1)].is_ascii_whitespace())
        {
            comment_start = index;
            break;
        }
    }
    let mut end = comment_start;
    while end > start && line[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    end
}

fn encode_value(value: &str, existing: &[u8]) -> String {
    if existing.starts_with(b"'") && existing.ends_with(b"'") && !value.contains('\'') {
        return format!("'{value}'");
    }
    if existing.starts_with(b"\"") && existing.ends_with(b"\"") {
        return double_quote(value);
    }
    if value.is_empty() {
        return String::new();
    }
    if value
        .bytes()
        .all(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'#' | b'\'' | b'"'))
    {
        return value.to_owned();
    }
    double_quote(value)
}

fn double_quote(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    format!("\"{escaped}\"")
}

fn decode_value(value: &[u8]) -> EnvResult<String> {
    let text = std::str::from_utf8(value)
        .map_err(|_| EnvError::invalid("환경변수 값을 UTF-8로 처리할 수 없습니다."))?;
    if text.len() >= 2 && text.starts_with('\'') && text.ends_with('\'') {
        return Ok(text[1..text.len() - 1].to_owned());
    }
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        let inner = &text[1..text.len() - 1];
        let mut decoded = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(character) = chars.next() {
            if character != '\\' {
                decoded.push(character);
                continue;
            }
            match chars.next() {
                Some('n') => decoded.push('\n'),
                Some('r') => decoded.push('\r'),
                Some('t') => decoded.push('\t'),
                Some('"') => decoded.push('"'),
                Some('\\') => decoded.push('\\'),
                Some(other) => {
                    decoded.push('\\');
                    decoded.push(other);
                }
                None => decoded.push('\\'),
            }
        }
        return Ok(decoded);
    }
    Ok(text.to_owned())
}

fn replace_span(source: &[u8], span: Span, replacement: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(source.len() - (span.end - span.start) + replacement.len());
    output.extend_from_slice(&source[..span.start]);
    output.extend_from_slice(replacement);
    output.extend_from_slice(&source[span.end..]);
    output
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    &bytes[start..]
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let bytes = trim_ascii_start(bytes);
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(0, |index| index + 1);
    &bytes[..end]
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn parse(source: &str) -> Document {
        Document::parse(source.as_bytes().to_vec(), Path::new("synthetic.env")).expect("parse")
    }

    #[test]
    fn round_trip_is_byte_identical() {
        let source = "# @group GPT\r\n\r\n# 설명\r\nexport GPT_API_KEY='fake_key' # keep\r\n";
        let document = parse(source);
        assert_eq!(document.source(), source.as_bytes());
        assert_eq!(document.newline_style(), NewlineStyle::CrLf);
        assert_eq!(document.assignments()[0].key, "GPT_API_KEY");
        let group_name = document.nodes().iter().find_map(|node| match node {
            Node::GroupDirective { name, .. } => Some(document.text(*name)),
            _ => None,
        });
        assert_eq!(group_name, Some("GPT"));
    }

    #[test]
    fn value_replacement_preserves_inline_comment() {
        let source = "PORT = fake_3000   # local port\n";
        let document = parse(source);
        let changed = document
            .replace_value("PORT", "fake_4000")
            .expect("replace");
        assert_eq!(
            String::from_utf8(changed).expect("utf8"),
            "PORT = fake_4000   # local port\n"
        );
    }

    #[test]
    fn replacement_quotes_unsafe_value() {
        let document = parse("NAME=fake_old\n");
        let changed = document
            .replace_value("NAME", "fake new # value")
            .expect("replace");
        assert_eq!(
            String::from_utf8(changed).expect("utf8"),
            "NAME=\"fake new # value\"\n"
        );
    }

    #[test]
    fn parses_multiline_double_quote() {
        let document = parse("CERT=\"fake_line_1\nfake_line_2\"\nPORT=fake_3000\n");
        assert_eq!(document.assignments().len(), 2);
        assert_eq!(document.assignments()[1].key, "PORT");
    }

    proptest::proptest! {
        #[test]
        fn arbitrary_utf8_source_never_panics(source in ".*") {
            let result = Document::parse(source.as_bytes().to_vec(), Path::new("generated.env"));
            if let Ok(document) = result {
                proptest::prop_assert_eq!(document.source(), source.as_bytes());
            }
        }
    }
}
