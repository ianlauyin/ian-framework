// The inherent impl of `{name}` comes after a trait impl.
// Put the inherent impl before the trait impls.
// Why a lint rule: The methods of a public type come before the trait impls.

use std::collections::BTreeSet;

use syn::spanned::Spanned;
use syn::{Item, Type, Visibility};

use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::file::source_order::Sort;
use crate::rule::rs::source::RsFile;

const RULE: &str = "impl-order";

pub(crate) enum Kind {
    Other,
    Inherent(String),
    Trait(String),
}

pub(crate) fn edit(file: &RsFile) -> Vec<crate::rule::edit::Edit> {
    crate::rule::rs::file::source_order::reorder(file, Sort::Impl)
}

pub(crate) fn check(file: &RsFile) -> Vec<Diagnostic> {
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    walk(&ast.items, &mut report);
    report.finish()
}

fn walk(items: &[Item], report: &mut RuleReport<'_>) {
    let public = public_types(items);
    let mut trait_seen = BTreeSet::new();
    for item in items {
        if let Item::Impl(imp) = item
            && let Some(name) = type_name(&imp.self_ty)
            && public.contains(&name)
        {
            if imp.trait_.is_none() {
                if trait_seen.contains(&name) {
                    report.at_span(
                        imp.span(),
                        format!("The inherent impl of `{name}` comes after a trait impl."),
                        "Put the inherent impl before the trait impls.",
                    );
                }
            } else {
                trait_seen.insert(name);
            }
        }
        if let Item::Mod(module) = item
            && let Some((_, nested)) = &module.content
        {
            walk(nested, report);
        }
    }
}

pub(crate) fn public_types(items: &[Item]) -> BTreeSet<String> {
    items
        .iter()
        .filter_map(|item| {
            let (name, vis) = match item {
                Item::Struct(item) => (item.ident.to_string(), &item.vis),
                Item::Enum(item) => (item.ident.to_string(), &item.vis),
                Item::Union(item) => (item.ident.to_string(), &item.vis),
                _ => return None,
            };
            if matches!(vis, Visibility::Inherited) {
                None
            } else {
                Some(name)
            }
        })
        .collect()
}

pub(crate) fn kind(item: &Item, public: &BTreeSet<String>) -> Kind {
    let Item::Impl(item) = item else {
        return Kind::Other;
    };
    let Some(name) = type_name(&item.self_ty) else {
        return Kind::Other;
    };
    if !public.contains(&name) {
        return Kind::Other;
    }
    if item.trait_.is_none() {
        Kind::Inherent(name)
    } else {
        Kind::Trait(name)
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
