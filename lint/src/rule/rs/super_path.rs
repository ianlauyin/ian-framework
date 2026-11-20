// The path uses `super::super`.
// Use a `crate::` path.
// Why a lint rule: A `super::super` path breaks when the module moves.

use std::fs;
use std::path::{Path, PathBuf};

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Path as SynPath, UseTree};

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "super-path";

pub fn check(path: &Path, _srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    check_source(path, &source)
}

pub(crate) fn check_source(path: &Path, source: &str) -> Vec<Diagnostic> {
    Finder::rust_source(path, source, RULE, |file, finder| {
        let mut pass = Pass { finder };
        pass.visit_file(file);
    })
}

struct Pass<'a, 'b> {
    finder: &'b mut Finder<'a>,
}

impl Visit<'_> for Pass<'_, '_> {
    fn visit_path(&mut self, path: &SynPath) {
        let mut previous = false;
        for segment in &path.segments {
            let is_super = segment.ident == "super";
            if previous && is_super {
                report(self.finder, path.span());
                break;
            }
            previous = is_super;
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_use_tree(&mut self, tree: &UseTree) {
        report_use(tree, false, self.finder);
    }
}

fn report_use(tree: &UseTree, previous: bool, finder: &mut Finder<'_>) {
    match tree {
        UseTree::Path(path) => {
            let is_super = path.ident == "super";
            if previous && is_super {
                report(finder, path.ident.span());
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
                report(finder, name.ident.span());
            }
        }
        UseTree::Rename(name) => {
            if previous && name.ident == "super" {
                report(finder, name.ident.span());
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn report(finder: &mut Finder<'_>, span: proc_macro2::Span) {
    finder.at_span(
        span,
        "The path uses `super::super`.",
        "Use a `crate::` path.",
    );
}
