// This `use` is out of import order.
// Put the other crates first, then `ian_framework_rs::`, then `super::`, `self::`, and the child modules, then `crate::`.
// Why a lint rule: That order keeps the imports from each source in one place.

use std::collections::BTreeSet;

use syn::spanned::Spanned;
use syn::{Item, ItemUse, UseTree, Visibility};

use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "import-order";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

const OTHER: u8 = 0;
const FRAMEWORK: u8 = 1;
const SUPER: u8 = 2;
const CRATE: u8 = 3;
const FIX: &str = "Put the other crates first, then `ian_framework_rs::`, then `super::`, `self::`, and the child modules, then `crate::`.";

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    walk(&ast.items, &mut report);
    report.finish()
}

fn walk(items: &[Item], report: &mut RuleReport<'_>) {
    let modules = module_names(items);
    let mut last_use: Option<u8> = None;
    let mut last_reexport: Option<u8> = None;
    for item in items {
        if let Item::Use(item) = item
            && let Some(group) = group(&item.tree, &modules)
        {
            let last = if matches!(item.vis, Visibility::Inherited) {
                &mut last_use
            } else {
                &mut last_reexport
            };
            check_use(item, group, last, report);
        }
        if let Item::Mod(module) = item
            && let Some((_, nested)) = &module.content
        {
            walk(nested, report);
        }
    }
}

fn check_use(item: &ItemUse, group: u8, last: &mut Option<u8>, report: &mut RuleReport<'_>) {
    if last.is_some_and(|previous| group < previous) {
        report.at_span(item.span(), "This `use` is out of import order.", FIX);
    }
    *last = Some(last.map_or(group, |previous| previous.max(group)));
}

fn group(tree: &UseTree, modules: &BTreeSet<String>) -> Option<u8> {
    let ident = match tree {
        UseTree::Path(path) => &path.ident,
        UseTree::Name(name) => &name.ident,
        UseTree::Rename(name) => &name.ident,
        UseTree::Glob(_) | UseTree::Group(_) => return None,
    };
    Some(if ident == "ian_framework_rs" {
        FRAMEWORK
    } else if ident == "super" || ident == "self" || modules.contains(&ident.to_string()) {
        SUPER
    } else if ident == "crate" {
        CRATE
    } else {
        OTHER
    })
}

fn module_names(items: &[Item]) -> BTreeSet<String> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Mod(module) => Some(module.ident.to_string()),
            _ => None,
        })
        .collect()
}
