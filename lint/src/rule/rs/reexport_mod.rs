// `{name}` is re-exported and is not `pub(crate) mod`.
// Declare `{name}` as `pub(crate) mod`.
// Why a lint rule: The caller uses the re-export. The child module stays private to the crate.

use std::collections::BTreeSet;

use syn::{Item, UseTree, Visibility};

use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::Report;
use crate::rule::rs::file::RsFile;

const RULE: &str = "reexport-mod";

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = Report::new(file.path, file.source, RULE);
    walk(&ast.items, &mut report);
    report.finish()
}

fn walk(items: &[Item], report: &mut Report<'_>) {
    let mut exported = BTreeSet::new();
    for item in items {
        if let Item::Use(item) = item
            && matches!(item.vis, Visibility::Public(_))
        {
            use_roots(&item.tree, &mut exported);
        }
    }
    for item in items {
        if let Item::Mod(module) = item {
            let name = module.ident.to_string();
            if exported.contains(&name) && !is_pub_crate(&module.vis) {
                report.at_span(
                    module.ident.span(),
                    format!("`{name}` is re-exported and is not `pub(crate) mod`."),
                    format!("Declare `{name}` as `pub(crate) mod`."),
                );
            }
            if let Some((_, nested)) = &module.content {
                walk(nested, report);
            }
        }
    }
}

fn use_roots(tree: &UseTree, names: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(path) => {
            if path.ident == "super" || path.ident == "crate" {
                return;
            }
            if path.ident == "self" {
                self_roots(&path.tree, names);
                return;
            }
            names.insert(path.ident.to_string());
        }
        UseTree::Group(group) => {
            for item in &group.items {
                use_roots(item, names);
            }
        }
        UseTree::Name(_) | UseTree::Rename(_) | UseTree::Glob(_) => {}
    }
}

fn self_roots(tree: &UseTree, names: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(path) => {
            names.insert(path.ident.to_string());
        }
        UseTree::Name(name) => {
            names.insert(name.ident.to_string());
        }
        UseTree::Rename(name) => {
            names.insert(name.ident.to_string());
        }
        UseTree::Group(group) => {
            for item in &group.items {
                self_roots(item, names);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn is_pub_crate(vis: &Visibility) -> bool {
    let Visibility::Restricted(restricted) = vis else {
        return false;
    };
    restricted.in_token.is_none()
        && restricted.path.segments.len() == 1
        && restricted.path.segments[0].ident == "crate"
}
