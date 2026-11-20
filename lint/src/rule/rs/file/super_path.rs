// The path uses `super::super`.
// Use a `crate::` path.
// Why a lint rule: A `super::super` path breaks when the module moves.

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Path as SynPath, UseTree};

use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::rule::RsRule;
use crate::rule::rs::source::RsFile;

const RULE: &str = "super-path";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub(crate) fn check(file: &RsFile) -> Vec<Diagnostic> {
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut pass = Pass {
        report: &mut report,
    };
    pass.visit_file(ast);
    report.finish()
}

struct Pass<'a, 'b> {
    report: &'b mut RuleReport<'a>,
}

impl Visit<'_> for Pass<'_, '_> {
    fn visit_path(&mut self, path: &SynPath) {
        let mut previous = false;
        for segment in &path.segments {
            let is_super = segment.ident == "super";
            if previous && is_super {
                report_super(self.report, path.span());
                break;
            }
            previous = is_super;
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_use_tree(&mut self, tree: &UseTree) {
        report_use(tree, false, self.report);
    }
}

fn report_use(tree: &UseTree, previous: bool, report: &mut RuleReport<'_>) {
    match tree {
        UseTree::Path(path) => {
            let is_super = path.ident == "super";
            if previous && is_super {
                report_super(report, path.ident.span());
            }
            report_use(&path.tree, is_super, report);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                report_use(item, previous, report);
            }
        }
        UseTree::Name(name) => {
            if previous && name.ident == "super" {
                report_super(report, name.ident.span());
            }
        }
        UseTree::Rename(name) => {
            if previous && name.ident == "super" {
                report_super(report, name.ident.span());
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn report_super(report: &mut RuleReport<'_>, span: proc_macro2::Span) {
    report.at_span(
        span,
        "The path uses `super::super`.",
        "Use a `crate::` path.",
    );
}
