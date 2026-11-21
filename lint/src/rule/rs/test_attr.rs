// A test is outside the `tests` folder.
// Move the test under the `tests` folder.
// Why a lint rule: Tests live in the `tests` folder. `mod tests` in `lib.rs` or `main.rs` stays.

use proc_macro2::{TokenStream, TokenTree};
use syn::spanned::Spanned;
use syn::{Attribute, ImplItem, Item, Meta, TraitItem};

use crate::file::rs::place::Place;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "test-attr";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);
const TEST_ATTRS: &[&str] = &["test", "rstest", "test_case"];

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let place = Place::new(file.path, file.srcs);
    if place.in_folder("tests") {
        return Vec::new();
    }
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    report_attrs(&ast.attrs, false, &mut report);
    walk(&ast.items, &place, &mut report);
    report.finish()
}

fn walk(items: &[Item], place: &Place, report: &mut RuleReport<'_>) {
    for item in items {
        let keep_cfg_test = is_crate_root(place) && is_mod_test(item);
        report_attrs(item_attrs(item), keep_cfg_test, report);
        match item {
            Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    walk(nested, place, report);
                }
            }
            Item::Impl(item) => {
                for impl_item in &item.items {
                    report_attrs(impl_attrs(impl_item), false, report);
                }
            }
            Item::Trait(item) => {
                for trait_item in &item.items {
                    report_attrs(trait_attrs(trait_item), false, report);
                }
            }
            _ => {}
        }
    }
}

fn report_attrs(attrs: &[Attribute], keep_cfg_test: bool, report: &mut RuleReport<'_>) {
    for attr in attrs {
        if keep_cfg_test && is_cfg_test(attr) {
            continue;
        }
        if is_test_attr(attr) {
            report.at_span(
                attr.span(),
                "A test is outside the `tests` folder.",
                "Move the test under the `tests` folder.",
            );
        }
    }
}

fn is_crate_root(place: &Place) -> bool {
    place.is_file("lib.rs") || place.is_file("main.rs")
}

fn is_mod_test(item: &Item) -> bool {
    matches!(item, Item::Mod(module) if module.ident == "tests")
}

fn is_test_attr(attr: &Attribute) -> bool {
    is_cfg_test(attr)
        || attr
            .path()
            .segments
            .last()
            .is_some_and(|segment| TEST_ATTRS.iter().any(|name| segment.ident == name))
}

fn is_cfg_test(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    match &attr.meta {
        Meta::List(list) => names_test(list.tokens.clone()),
        _ => false,
    }
}

fn names_test(tokens: TokenStream) -> bool {
    let mut negated = false;
    for tree in tokens {
        match tree {
            TokenTree::Ident(ident) => {
                if ident == "test" {
                    return true;
                }
                negated = ident == "not";
            }
            TokenTree::Group(group) => {
                if !negated && names_test(group.stream()) {
                    return true;
                }
                negated = false;
            }
            _ => negated = false,
        }
    }
    false
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
        _ => &[],
    }
}

fn impl_attrs(item: &ImplItem) -> &[Attribute] {
    match item {
        ImplItem::Const(item) => &item.attrs,
        ImplItem::Fn(item) => &item.attrs,
        ImplItem::Type(item) => &item.attrs,
        ImplItem::Macro(item) => &item.attrs,
        _ => &[],
    }
}

fn trait_attrs(item: &TraitItem) -> &[Attribute] {
    match item {
        TraitItem::Const(item) => &item.attrs,
        TraitItem::Fn(item) => &item.attrs,
        TraitItem::Type(item) => &item.attrs,
        TraitItem::Macro(item) => &item.attrs,
        _ => &[],
    }
}
