// This item is out of source order.
// Put `mod` and `pub(crate) mod` first, then `use`, then `pub mod`, then `pub use` and `pub(crate) use`, then a private `static`, then constants, then public items, then private items.
// Why a lint rule: That order keeps each kind of item in one place.

use std::collections::BTreeSet;

use syn::spanned::Spanned;
use syn::{Item, Type, Visibility};

use crate::rule::diagnostic::Diagnostic;
use crate::rule::edit::Edit;
use crate::rule::report::Report;
use crate::rule::rs::file::RsFile;
use crate::rule::rs::impl_order::Kind;

const RULE: &str = "source-order";
const MOD: u8 = 0;
const USE: u8 = 1;
const PUB_MOD: u8 = 2;
const REEXPORT: u8 = 3;
const STATIC: u8 = 4;
const CONSTANT: u8 = 5;
const PUBLIC: u8 = 6;
const PRIVATE: u8 = 7;
const FIX: &str = "Put `mod` and `pub(crate) mod` first, then `use`, then `pub mod`, then `pub use` and `pub(crate) use`, then a private `static`, then constants, then public items, then private items.";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sort {
    Source,
    Impl,
}

pub(crate) fn edit(file: &RsFile) -> Vec<Edit> {
    reorder(file, Sort::Source)
}

pub(crate) fn reorder(file: &RsFile, sort: Sort) -> Vec<Edit> {
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let Some((start, end, text)) = rewrite(file.source, &ast.items, sort) else {
        return Vec::new();
    };
    vec![Edit {
        path: file.path.to_path_buf(),
        start: start as u32,
        end: end as u32,
        text,
    }]
}

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = Report::new(file.path, file.source, RULE);
    walk(&ast.items, &mut report);
    report.finish()
}

fn walk(items: &[Item], report: &mut Report<'_>) {
    let private_types = private_type_names(items);
    let mut last: Option<u8> = None;
    for item in items {
        let group = group(item, &private_types);
        if last.is_some_and(|previous| group < previous) {
            report.at_span(item.span(), "This item is out of source order.", FIX);
        }
        last = Some(last.map_or(group, |previous| previous.max(group)));
        if let Item::Mod(module) = item
            && let Some((_, nested)) = &module.content
        {
            walk(nested, report);
        }
    }
}

pub(crate) fn group(item: &Item, private_types: &BTreeSet<String>) -> u8 {
    match item {
        Item::Mod(item) => {
            if is_public(&item.vis) {
                PUB_MOD
            } else {
                MOD
            }
        }
        Item::Use(item) => {
            if is_private(&item.vis) {
                USE
            } else {
                REEXPORT
            }
        }
        Item::ExternCrate(item) => {
            if is_private(&item.vis) {
                USE
            } else {
                REEXPORT
            }
        }
        Item::Static(_) => STATIC,
        Item::Const(_) => CONSTANT,
        Item::Impl(item) => {
            if type_name(&item.self_ty).is_some_and(|name| private_types.contains(&name)) {
                PRIVATE
            } else {
                PUBLIC
            }
        }
        Item::Macro(item) => {
            if item
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("macro_export"))
            {
                PUBLIC
            } else {
                PRIVATE
            }
        }
        _ => {
            if vis_of(item).is_some_and(|vis| !is_private(vis)) {
                PUBLIC
            } else {
                PRIVATE
            }
        }
    }
}

pub(crate) fn private_type_names(items: &[Item]) -> BTreeSet<String> {
    items
        .iter()
        .filter_map(|item| {
            let (name, vis) = type_item(item)?;
            if is_private(vis) { Some(name) } else { None }
        })
        .collect()
}

fn type_item(item: &Item) -> Option<(String, &Visibility)> {
    match item {
        Item::Struct(item) => Some((item.ident.to_string(), &item.vis)),
        Item::Enum(item) => Some((item.ident.to_string(), &item.vis)),
        Item::Union(item) => Some((item.ident.to_string(), &item.vis)),
        _ => None,
    }
}

fn type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Type::Group(ty) => type_name(&ty.elem),
        Type::Paren(ty) => type_name(&ty.elem),
        _ => None,
    }
}

fn vis_of(item: &Item) -> Option<&Visibility> {
    match item {
        Item::Const(item) => Some(&item.vis),
        Item::Enum(item) => Some(&item.vis),
        Item::ExternCrate(item) => Some(&item.vis),
        Item::Fn(item) => Some(&item.vis),
        Item::Mod(item) => Some(&item.vis),
        Item::Static(item) => Some(&item.vis),
        Item::Struct(item) => Some(&item.vis),
        Item::Trait(item) => Some(&item.vis),
        Item::TraitAlias(item) => Some(&item.vis),
        Item::Type(item) => Some(&item.vis),
        Item::Union(item) => Some(&item.vis),
        Item::Use(item) => Some(&item.vis),
        _ => None,
    }
}

fn is_public(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

fn is_private(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Inherited)
}

fn rewrite(source: &str, items: &[Item], sort: Sort) -> Option<(usize, usize, String)> {
    if items.is_empty() {
        return None;
    }
    let private_types = private_type_names(items);
    let public_types = crate::rule::rs::impl_order::public_types(items);
    let spans: Vec<(usize, usize)> = items.iter().map(item_bounds).collect();
    let (start, end, ranges) = crate::rule::edit::piece_ranges(source, &spans)?;
    let mut pieces = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let (from, to) = ranges[index];
        let group = if sort == Sort::Source {
            group(item, &private_types)
        } else {
            0
        };
        let kind = if sort == Sort::Impl {
            crate::rule::rs::impl_order::kind(item, &public_types)
        } else {
            Kind::Other
        };
        pieces.push(Piece {
            group,
            index,
            text: piece_text(source, from, to, item, sort),
            kind,
        });
    }
    if sort == Sort::Source {
        pieces.sort_by(|left, right| {
            left.group
                .cmp(&right.group)
                .then(left.index.cmp(&right.index))
        });
    }
    if sort == Sort::Impl {
        move_inherent_impls(&mut pieces);
    }
    let text: String = pieces.iter().map(|piece| piece.text.as_str()).collect();
    if text == source[start..end] {
        None
    } else {
        Some((start, end, text))
    }
}

fn piece_text(source: &str, from: usize, to: usize, item: &Item, sort: Sort) -> String {
    let Item::Mod(module) = item else {
        return source[from..to].to_owned();
    };
    let Some((_, nested)) = &module.content else {
        return source[from..to].to_owned();
    };
    let Some((inner_start, inner_end, inner)) = rewrite(source, nested, sort) else {
        return source[from..to].to_owned();
    };
    if inner_start < from || inner_end > to || inner_start > inner_end {
        return source[from..to].to_owned();
    }
    let mut text = String::new();
    text.push_str(&source[from..inner_start]);
    text.push_str(&inner);
    text.push_str(&source[inner_end..to]);
    text
}

fn move_inherent_impls(pieces: &mut Vec<Piece>) {
    let names = pieces
        .iter()
        .filter_map(|piece| kind_name(&piece.kind))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    for name in names {
        loop {
            let Some(trait_at) = pieces.iter().position(|piece| is_trait(&piece.kind, &name))
            else {
                break;
            };
            let Some(inherent_at) = pieces.iter().enumerate().find_map(|(index, piece)| {
                (index > trait_at && is_inherent(&piece.kind, &name)).then_some(index)
            }) else {
                break;
            };
            let piece = pieces.remove(inherent_at);
            pieces.insert(trait_at, piece);
        }
    }
}

fn kind_name(kind: &Kind) -> Option<&str> {
    match kind {
        Kind::Inherent(name) | Kind::Trait(name) => Some(name),
        Kind::Other => None,
    }
}

fn is_inherent(kind: &Kind, name: &str) -> bool {
    matches!(kind, Kind::Inherent(found) if found == name)
}

fn is_trait(kind: &Kind, name: &str) -> bool {
    matches!(kind, Kind::Trait(found) if found == name)
}

fn item_bounds(item: &Item) -> (usize, usize) {
    let range = item.span().byte_range();
    (range.start, range.end)
}

struct Piece {
    group: u8,
    index: usize,
    text: String,
    kind: Kind,
}
