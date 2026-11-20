use std::path::Path;

use lightningcss::stylesheet::ParserOptions;
use lightningcss::stylesheet::StyleSheet;

use crate::rule::fe::css::position::Map;
use crate::rule::fe::css::scan::{Bounds, bounds, char_len, comment_end, skip_ws, starts_comment};
use crate::rule::fe::css::sheet::child_items;

pub(crate) struct Item {
    pub(crate) kind: Kind,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

pub(crate) enum Kind {
    Comment,
    Declaration,
    Rule {
        selector_end: u32,
        children: Vec<Item>,
    },
    AtRule {
        brace: Option<u32>,
        children: Vec<Item>,
    },
}

impl Item {
    pub(crate) fn is_comment(&self) -> bool {
        matches!(self.kind, Kind::Comment)
    }
}

pub(crate) fn parse(source: &str) -> Vec<Item> {
    let bom = if source.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let source = &source[bom..];
    let options = ParserOptions {
        error_recovery: true,
        ..ParserOptions::default()
    };
    let map = Map::new(source);
    let rules = match StyleSheet::parse(source, options) {
        Ok(sheet) => child_items(&sheet.rules, &map),
        Err(_) => Vec::new(),
    };
    let mut items = merge(&map, rules, 0, source.len());
    shift(&mut items, bom as u32);
    items
}

pub(crate) fn is_css_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
}

pub(crate) fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

pub(crate) fn merge(
    map: &Map<'_>,
    mut rules: Vec<Item>,
    region_start: usize,
    region_end: usize,
) -> Vec<Item> {
    rules.sort_by_key(|item| item.start);
    let mut items = Vec::new();
    let mut cursor = region_start;
    for rule in rules {
        let start = rule.start as usize;
        let end = rule.end as usize;
        if start < cursor || end > region_end || end <= start {
            continue;
        }
        if start > cursor {
            items.extend(parse_gap(map, cursor, start));
        }
        cursor = end;
        items.push(rule);
    }
    if region_end > cursor {
        items.extend(parse_gap(map, cursor, region_end));
    }
    items
}

fn parse_gap(map: &Map<'_>, from: usize, to: usize) -> Vec<Item> {
    let mut items = Vec::new();
    let mut index = from;
    while index < to {
        index = skip_ws(map.source, index, to);
        if index >= to {
            break;
        }
        if starts_comment(map.source, index) {
            let end = comment_end(map.source, index, to);
            items.push(Item {
                kind: Kind::Comment,
                start: index as u32,
                end: end as u32,
            });
            index = end;
            continue;
        }
        let at = map.source.as_bytes()[index] == b'@';
        let item = match bounds(map.source, index, to) {
            Bounds::Statement { end } if end <= index => {
                index += char_len(map.source, index);
                continue;
            }
            Bounds::Statement { end } => Item {
                kind: if at {
                    Kind::AtRule {
                        brace: None,
                        children: Vec::new(),
                    }
                } else {
                    Kind::Declaration
                },
                start: index as u32,
                end: end as u32,
            },
            Bounds::Block {
                brace,
                inner_end,
                end,
            } => {
                let children = merge(map, Vec::new(), brace + 1, inner_end);
                Item {
                    kind: if at {
                        Kind::AtRule {
                            brace: Some(brace as u32),
                            children,
                        }
                    } else {
                        Kind::Rule {
                            selector_end: brace as u32,
                            children,
                        }
                    },
                    start: index as u32,
                    end: end as u32,
                }
            }
        };
        index = item.end as usize;
        items.push(item);
    }
    items
}

fn shift(items: &mut [Item], by: u32) {
    if by == 0 {
        return;
    }
    for item in items {
        item.start += by;
        item.end += by;
        match &mut item.kind {
            Kind::Rule {
                selector_end,
                children,
            } => {
                *selector_end += by;
                shift(children, by);
            }
            Kind::AtRule { brace, children } => {
                if let Some(brace) = brace {
                    *brace += by;
                }
                shift(children, by);
            }
            Kind::Comment | Kind::Declaration => {}
        }
    }
}
