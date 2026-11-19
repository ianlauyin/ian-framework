// This item is out of source order.
// Put `mod` and `pub(crate) mod` first, then `use`, then `pub mod`, then `pub use` and `pub(crate) use`, then a private `static`, then constants, then public items, then private items.
// Why a lint rule: That order keeps each kind of item in one place.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use syn::spanned::Spanned;
use syn::{Item, Type, Visibility};

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

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

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    check_source(path, &source)
}

pub(crate) fn check_source(path: &Path, source: &str) -> Vec<Diagnostic> {
    Finder::rust_source(path, source, RULE, |file, finder| {
        walk(&file.items, finder);
    })
}

fn walk(items: &[Item], finder: &mut Finder<'_>) {
    let private_types = private_type_names(items);
    let mut last: Option<u8> = None;
    for item in items {
        let group = group(item, &private_types);
        if last.is_some_and(|previous| group < previous) {
            finder.at_span(item.span(), "This item is out of source order.", FIX);
        }
        last = Some(last.map_or(group, |previous| previous.max(group)));
        if let Item::Mod(module) = item
            && let Some((_, nested)) = &module.content
        {
            walk(nested, finder);
        }
    }
}

fn group(item: &Item, private_types: &BTreeSet<String>) -> u8 {
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

fn private_type_names(items: &[Item]) -> BTreeSet<String> {
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
