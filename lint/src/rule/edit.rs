use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Failure;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Edit {
    pub(crate) path: PathBuf,
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) text: String,
}

#[derive(Debug)]
pub(crate) enum ApplyError {
    Overlap,
    Range,
}

pub(crate) fn replace(source: &str, edits: &[Edit]) -> Result<String, ApplyError> {
    let refs: Vec<_> = edits.iter().collect();
    apply(source, &refs)
}

pub(crate) fn failure(path: &Path, error: ApplyError) -> Failure {
    let detail = match error {
        ApplyError::Overlap => "the edits overlap.",
        ApplyError::Range => "an edit is outside the file.",
    };
    Failure {
        output: format!("{}: {detail}", path.display()),
    }
}

pub(crate) fn commit(path: &Path, original: &str, edited: &str) -> Vec<Failure> {
    if edited == original {
        return Vec::new();
    }
    match fs::write(path, edited) {
        Ok(()) => Vec::new(),
        Err(error) => vec![Failure {
            output: format!("{}: {error}", path.display()),
        }],
    }
}

pub(crate) fn apply(source: &str, edits: &[&Edit]) -> Result<String, ApplyError> {
    let mut ordered: Vec<&Edit> = edits.to_vec();
    ordered.sort_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)));
    for pair in ordered.windows(2) {
        if pair[0].end > pair[1].start {
            return Err(ApplyError::Overlap);
        }
    }
    for edit in &ordered {
        let start = edit.start as usize;
        let end = edit.end as usize;
        if start > end
            || end > source.len()
            || !source.is_char_boundary(start)
            || !source.is_char_boundary(end)
        {
            return Err(ApplyError::Range);
        }
    }
    let mut next = source.to_owned();
    for edit in ordered.iter().rev() {
        next.replace_range(edit.start as usize..edit.end as usize, &edit.text);
    }
    Ok(next)
}

pub(crate) fn write(edits: &[Edit]) -> Vec<Failure> {
    let mut by_path: BTreeMap<&Path, Vec<&Edit>> = BTreeMap::new();
    for edit in edits {
        by_path.entry(edit.path.as_path()).or_default().push(edit);
    }
    let mut failures = Vec::new();
    for (path, file_edits) in by_path {
        let Ok(source) = fs::read_to_string(path) else {
            failures.push(Failure {
                output: format!("{}: the file was not read.", path.display()),
            });
            continue;
        };
        match apply(&source, &file_edits) {
            Ok(next) if next == source => {}
            Ok(next) => {
                if let Err(error) = fs::write(path, next) {
                    failures.push(Failure {
                        output: format!("{}: {error}", path.display()),
                    });
                }
            }
            Err(ApplyError::Overlap) => failures.push(Failure {
                output: format!("{}: the edits overlap.", path.display()),
            }),
            Err(ApplyError::Range) => failures.push(Failure {
                output: format!("{}: an edit is outside the file.", path.display()),
            }),
        }
    }
    failures
}

pub(crate) fn piece_ranges(
    source: &str,
    spans: &[(usize, usize)],
) -> Option<(usize, usize, Vec<(usize, usize)>)> {
    if spans.is_empty() {
        return None;
    }
    let mut ranges = Vec::with_capacity(spans.len());
    let mut cursor = line_start(source, spans[0].0);
    let region_start = cursor;
    for (index, &(_, end)) in spans.iter().enumerate() {
        let is_last = index + 1 == spans.len();
        let limit = if let Some(&(next, _)) = spans.get(index + 1) {
            line_start(source, next)
        } else {
            source.len()
        };
        let to = line_end(source, end, limit.max(cursor), is_last).max(cursor);
        ranges.push((cursor, to));
        cursor = to;
    }
    Some((region_start, cursor, ranges))
}

fn line_start(source: &str, offset: usize) -> usize {
    let offset = offset.min(source.len());
    match source[..offset].rfind('\n') {
        Some(newline) => {
            if source[newline + 1..offset].bytes().all(is_indent) {
                newline + 1
            } else {
                offset
            }
        }
        None => {
            if source[..offset].bytes().all(is_indent) {
                0
            } else {
                offset
            }
        }
    }
}

fn line_end(source: &str, item_end: usize, limit: usize, is_last: bool) -> usize {
    let limit = limit.min(source.len());
    let mut end = item_end.min(limit);
    let rest = &source[end..limit];
    let Some(relative) = rest.find('\n') else {
        if is_last { return end } else { return limit }
    };
    end += relative + 1;
    while end < limit {
        let rest = &source[end..limit];
        let Some(relative) = rest.find('\n') else {
            break;
        };
        let line = &rest[..relative];
        if !line.bytes().all(|byte| is_indent(byte) || byte == b'\r') {
            break;
        }
        end += relative + 1;
    }
    end
}

fn is_indent(byte: u8) -> bool {
    byte == b' ' || byte == b'\t'
}
