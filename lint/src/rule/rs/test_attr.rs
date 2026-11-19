// A test is outside the test folder.
// Move the test under the test folder.
// Why a lint rule: Tests live in the test folder. `mod test` in `lib.rs` or `main.rs` stays.

use std::fs;
use std::path::{Path, PathBuf};

use syn::spanned::Spanned;
use syn::{Attribute, ImplItem, Item, Meta, TraitItem};

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;
use crate::rule::rs::place::Place;

const RULE: &str = "test-attr";

pub fn check(path: &Path, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    check_source(path, &source, srcs)
}

pub(crate) fn check_source(path: &Path, source: &str, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let place = Place::new(path, srcs);
    if place.in_test() {
        return Vec::new();
    }
    Finder::rust_source(path, source, RULE, |file, finder| {
        report_attrs(&file.attrs, false, finder);
        walk(&file.items, &place, finder);
    })
}

fn walk(items: &[Item], place: &Place, finder: &mut Finder<'_>) {
    for item in items {
        let keep_cfg_test = is_crate_root(&place) && is_mod_test(item);
        report_attrs(item_attrs(item), keep_cfg_test, finder);
        match item {
            Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    walk(nested, place, finder);
                }
            }
            Item::Impl(item) => {
                for impl_item in &item.items {
                    report_attrs(impl_attrs(impl_item), false, finder);
                }
            }
            Item::Trait(item) => {
                for trait_item in &item.items {
                    report_attrs(trait_attrs(trait_item), false, finder);
                }
            }
            _ => {}
        }
    }
}

fn report_attrs(attrs: &[Attribute], keep_cfg_test: bool, finder: &mut Finder<'_>) {
    for attr in attrs {
        if keep_cfg_test && is_cfg_test(attr) {
            continue;
        }
        if is_test_attr(attr) {
            finder.at_span(
                attr.span(),
                "A test is outside the test folder.",
                "Move the test under the test folder.",
            );
        }
    }
}

fn is_crate_root(place: &Place) -> bool {
    place.is_file("lib.rs") || place.is_file("main.rs")
}

fn is_mod_test(item: &Item) -> bool {
    matches!(item, Item::Mod(module) if module.ident == "test")
}

fn is_test_attr(attr: &Attribute) -> bool {
    if attr.path().is_ident("test") || is_cfg_test(attr) {
        return true;
    }
    let segments: Vec<_> = attr
        .path()
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    segments == ["tokio", "test"]
}

fn is_cfg_test(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    match &attr.meta {
        Meta::List(list) => list.tokens.to_string() == "test",
        _ => false,
    }
}

fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn impl_attrs(item: &ImplItem) -> &[Attribute] {
    match item {
        ImplItem::Const(item) => &item.attrs,
        ImplItem::Fn(item) => &item.attrs,
        ImplItem::Type(item) => &item.attrs,
        ImplItem::Macro(item) => &item.attrs,
        ImplItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn trait_attrs(item: &TraitItem) -> &[Attribute] {
    match item {
        TraitItem::Const(item) => &item.attrs,
        TraitItem::Fn(item) => &item.attrs,
        TraitItem::Type(item) => &item.attrs,
        TraitItem::Macro(item) => &item.attrs,
        TraitItem::Verbatim(_) => &[],
        _ => &[],
    }
}
