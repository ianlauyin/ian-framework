// The inherent impl of `{name}` is outside the file that defines `{name}`.
// Move the inherent impl into the file that defines `{name}`.
// Why a lint rule: The type and its methods stay in one file. A trait impl can be in another file.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use syn::spanned::Spanned;
use syn::{Item, Type};

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "inherent-impl";

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
    let mut defined = BTreeSet::new();
    collect_types(&file.items, &mut defined);
    let mut finder = Finder::new(path, source);
    walk(&file.items, &defined, &mut finder);
    finder.finish()
}

fn collect_types(items: &[Item], defined: &mut BTreeSet<String>) {
    for item in items {
        match item {
            Item::Struct(item) => {
                defined.insert(item.ident.to_string());
            }
            Item::Enum(item) => {
                defined.insert(item.ident.to_string());
            }
            Item::Union(item) => {
                defined.insert(item.ident.to_string());
            }
            Item::Mod(item) => {
                if let Some((_, nested)) = &item.content {
                    collect_types(nested, defined);
                }
            }
            _ => {}
        }
    }
}

fn walk(items: &[Item], defined: &BTreeSet<String>, finder: &mut Finder<'_>) {
    for item in items {
        if let Item::Impl(item) = item
            && item.trait_.is_none()
            && let Some(name) = type_name(&item.self_ty)
            && !defined.contains(&name)
        {
            finder.report(
                item.span().byte_range().start as u32,
                RULE,
                format!("The inherent impl of `{name}` is outside the file that defines `{name}`."),
                format!("Move the inherent impl into the file that defines `{name}`."),
            );
        }
        if let Item::Mod(item) = item
            && let Some((_, nested)) = &item.content
        {
            walk(nested, defined, finder);
        }
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
