// `{name}` is not a private static.
// Remove the visibility.
// Why a lint rule: A static is private. Visibility on a static exposes the value.

use syn::spanned::Spanned;

use crate::diagnostic::Diagnostic;
use crate::rule::report::Report;
use crate::rule::rs::file::RsFile;

const RULE: &str = "private-static";

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = Report::new(file.path, file.source, RULE);
    walk(&ast.items, &mut report);
    report.finish()
}

fn walk(items: &[syn::Item], report: &mut Report<'_>) {
    for item in items {
        match item {
            syn::Item::Static(item) => {
                report_static(&item.vis, &item.ident, report);
            }
            syn::Item::ForeignMod(item) => {
                for foreign in &item.items {
                    let syn::ForeignItem::Static(item) = foreign else {
                        continue;
                    };
                    report_static(&item.vis, &item.ident, report);
                }
            }
            syn::Item::Mod(item) => {
                let Some((_, nested)) = &item.content else {
                    continue;
                };
                walk(nested, report);
            }
            _ => {}
        }
    }
}

fn report_static(vis: &syn::Visibility, ident: &syn::Ident, report: &mut Report<'_>) {
    if matches!(vis, syn::Visibility::Inherited) {
        return;
    }
    report.at_span(
        vis.span(),
        format!("`{ident}` is not a private static."),
        "Remove the visibility.",
    );
}
