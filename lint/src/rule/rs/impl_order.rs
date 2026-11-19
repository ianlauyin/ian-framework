// The inherent impl of `{name}` comes after a trait impl.
// Put the inherent impl before the trait impls.
// Why a lint rule: The methods of a public type come before the trait impls.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use syn::spanned::Spanned;
use syn::{Item, Type, Visibility};

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "impl-order";

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    check_source(path, &source)
}

pub(crate) fn check_source(path: &Path, source: &str) -> Vec<Diagnostic> {
    let Ok(file) = syn::parse_file(source) else {
        return Vec::new();
    };
    let mut finder = Finder::new(path, source);
    walk(&file.items, &mut finder);
    finder.finish()
}

fn walk(items: &[Item], finder: &mut Finder<'_>) {
    let public = public_types(items);
    let mut trait_seen = BTreeSet::new();
    for item in items {
        if let Item::Impl(imp) = item
            && let Some(name) = type_name(&imp.self_ty)
            && public.contains(&name)
        {
            if imp.trait_.is_none() {
                if trait_seen.contains(&name) {
                    finder.report(
                        imp.span().byte_range().start as u32,
                        RULE,
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
            walk(nested, finder);
        }
    }
}

fn public_types(items: &[Item]) -> BTreeSet<String> {
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
