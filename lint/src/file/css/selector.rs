use crate::file::css::parse::is_css_whitespace;
use crate::file::css::scan::{char_len, comment_end, string_end};

pub(crate) fn selectors(selector: &str) -> Vec<(usize, &str)> {
    split_commas(selector)
        .into_iter()
        .map(|(offset, part)| strip(offset, part))
        .collect()
}

fn split_commas(selector: &str) -> Vec<(usize, &str)> {
    let bytes = selector.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let mut paren = 0u32;
    let mut bracket = 0u32;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\\' {
            index += 1;
            if index < bytes.len() {
                index += char_len(selector, index);
            }
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            index = string_end(selector, index, selector.len());
            continue;
        }
        if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'*' {
            index = comment_end(selector, index, selector.len());
            continue;
        }
        if paren == 0 && bracket == 0 && byte == b',' {
            parts.push((start, &selector[start..index]));
            index += 1;
            start = index;
            continue;
        }
        match byte {
            b'(' => paren += 1,
            b')' => paren = paren.saturating_sub(1),
            b'[' => bracket += 1,
            b']' => bracket = bracket.saturating_sub(1),
            _ => {}
        }
        index += char_len(selector, index);
    }
    parts.push((start, &selector[start..]));
    parts
}

fn strip(part_start: usize, part: &str) -> (usize, &str) {
    let mut text = part;
    let mut offset = part_start;
    loop {
        let trimmed = text.trim_start_matches(is_css_ws_char);
        offset += text.len() - trimmed.len();
        text = trimmed;
        let Some(rest) = strip_leading_comment(text) else {
            break;
        };
        if rest.len() == text.len() {
            break;
        }
        offset += text.len() - rest.len();
        text = rest;
    }
    loop {
        text = text.trim_end_matches(is_css_ws_char);
        let Some(rest) = strip_trailing_comment(text) else {
            break;
        };
        if rest.len() == text.len() {
            break;
        }
        text = rest;
    }
    (offset, text)
}

fn strip_leading_comment(text: &str) -> Option<&str> {
    if !text.starts_with("/*") {
        return None;
    }
    Some(&text[comment_end(text, 0, text.len())..])
}

fn strip_trailing_comment(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if !text.ends_with("*/") {
        return None;
    }
    let mut index = 0;
    let mut comment_start = None;
    while index + 1 < bytes.len() {
        if bytes[index] == b'"' || bytes[index] == b'\'' {
            index = string_end(text, index, text.len());
            continue;
        }
        if bytes[index] == b'/' && bytes[index + 1] == b'*' {
            let start = index;
            index = comment_end(text, index, text.len());
            if index == bytes.len() {
                comment_start = Some(start);
                break;
            }
            continue;
        }
        index += char_len(text, index);
    }
    comment_start.map(|start| &text[..start])
}

pub(crate) fn is_css_ws_char(char: char) -> bool {
    char.is_ascii() && is_css_whitespace(char as u8)
}

pub(crate) fn classes(selector: &str) -> Vec<&str> {
    let bytes = selector.as_bytes();
    let mut names = Vec::new();
    let mut index = 0;
    let mut bracket = 0u32;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'"' || byte == b'\'' {
            index = string_end(selector, index, selector.len());
            continue;
        }
        if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'*' {
            index = comment_end(selector, index, selector.len());
            continue;
        }
        match byte {
            b'[' => bracket += 1,
            b']' => bracket = bracket.saturating_sub(1),
            b'.' if bracket == 0 => {
                let start = index + 1;
                let end = name_end(selector, start);
                if end > start && !bytes[start].is_ascii_digit() {
                    names.push(&selector[start..end]);
                }
                index = end.max(start);
                continue;
            }
            _ => {}
        }
        index += char_len(selector, index);
    }
    names
}

pub(crate) fn name_end(selector: &str, start: usize) -> usize {
    selector[start..]
        .find(|char: char| !(char.is_alphanumeric() || char == '-' || char == '_'))
        .map_or(selector.len(), |offset| start + offset)
}
