// The inherent impl of `{name}` comes after a trait impl.
// Put the inherent impl before the trait impls.
// Why a lint rule: The methods of a public type come before the trait impls.

use std::collections::BTreeSet;

use syn::spanned::Spanned;
use syn::{Item, Visibility};

use crate::file::rs::item::type_name;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "impl-order";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
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
