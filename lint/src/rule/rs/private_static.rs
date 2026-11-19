// `{name}` is not a private static.
// Remove the visibility.
// Why a lint rule: A static is private. Visibility on a static exposes the value.

use std::fs;
use std::path::Path;

use syn::spanned::Spanned;

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "private-static";

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    Finder::rust_source(path, &source, RULE, |file, finder| {
        walk(&file.items, finder);
    })
}

fn walk(items: &[syn::Item], finder: &mut Finder<'_>) {
    for item in items {
        match item {
            syn::Item::Static(item) => {
                report(&item.vis, &item.ident, finder);
            }
            syn::Item::ForeignMod(item) => {
                for foreign in &item.items {
                    let syn::ForeignItem::Static(item) = foreign else {
                        continue;
                    };
                    report(&item.vis, &item.ident, finder);
                }
            }
            syn::Item::Mod(item) => {
                let Some((_, nested)) = &item.content else {
                    continue;
                };
                walk(nested, finder);
            }
            _ => {}
        }
    }
}

fn report(vis: &syn::Visibility, ident: &syn::Ident, finder: &mut Finder<'_>) {
    if matches!(vis, syn::Visibility::Inherited) {
        return;
    }
    finder.at_span(
        vis.span(),
        format!("`{ident}` is not a private static."),
        "Remove the visibility.",
    );
}
