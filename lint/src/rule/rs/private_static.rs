// `{name}` is not a private static.
// Remove the visibility.
// Why a lint rule: A static is private. Visibility on a static exposes the value.

use std::fs;
use std::path::Path;

use syn::spanned::Spanned;

use crate::diagnostic::{Diagnostic, suppress};

const RULE: &str = "private-static";

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(file) = syn::parse_file(&source) else {
        return Vec::new();
    };
    let mut diagnostics = Vec::new();
    walk(&file.items, path, &source, &mut diagnostics);
    suppress(&source, diagnostics)
}

fn walk(items: &[syn::Item], path: &Path, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    for item in items {
        match item {
            syn::Item::Static(item) => {
                report(path, source, &item.vis, &item.ident, diagnostics);
            }
            syn::Item::ForeignMod(item) => {
                for foreign in &item.items {
                    let syn::ForeignItem::Static(item) = foreign else {
                        continue;
                    };
                    report(path, source, &item.vis, &item.ident, diagnostics);
                }
            }
            syn::Item::Mod(item) => {
                let Some((_, nested)) = &item.content else {
                    continue;
                };
                walk(nested, path, source, diagnostics);
            }
            _ => {}
        }
    }
}

fn report(
    path: &Path,
    source: &str,
    vis: &syn::Visibility,
    ident: &syn::Ident,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if matches!(vis, syn::Visibility::Inherited) {
        return;
    }
    diagnostics.push(Diagnostic::at_offset(
        path,
        source,
        vis.span().byte_range().start as u32,
        RULE,
        format!("`{ident}` is not a private static."),
        "Remove the visibility.",
    ));
}
