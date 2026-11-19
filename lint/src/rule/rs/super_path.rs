// The path uses `super::super`.
// Use a `crate::` path.
// Why a lint rule: A `super::super` path breaks when the module moves.

use std::fs;
use std::path::Path;

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Path as SynPath, UseTree};

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "super-path";

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
    let mut pass = Pass {
        finder: Finder::new(path, source),
    };
    pass.visit_file(&file);
    pass.finder.finish()
}

struct Pass<'a> {
    finder: Finder<'a>,
}

impl Visit<'_> for Pass<'_> {
    fn visit_path(&mut self, path: &SynPath) {
        let mut previous = false;
        for segment in &path.segments {
            let is_super = segment.ident == "super";
            if previous && is_super {
                report(&mut self.finder, path.span().byte_range().start as u32);
                break;
            }
            previous = is_super;
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_use_tree(&mut self, tree: &UseTree) {
        report_use(tree, false, &mut self.finder);
    }
}

fn report_use(tree: &UseTree, previous: bool, finder: &mut Finder<'_>) {
    match tree {
        UseTree::Path(path) => {
            let is_super = path.ident == "super";
            if previous && is_super {
                report(finder, path.ident.span().byte_range().start as u32);
            }
            report_use(&path.tree, is_super, finder);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                report_use(item, previous, finder);
            }
        }
        UseTree::Name(name) => {
            if previous && name.ident == "super" {
                report(finder, name.ident.span().byte_range().start as u32);
            }
        }
        UseTree::Rename(name) => {
            if previous && name.ident == "super" {
                report(finder, name.ident.span().byte_range().start as u32);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn report(finder: &mut Finder<'_>, offset: u32) {
    finder.report(
        offset,
        RULE,
        "The path uses `super::super`.",
        "Use a `crate::` path.",
    );
}
