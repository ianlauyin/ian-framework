// A test file uses a `super` path.
// Use a `crate::` path.
// Why a lint rule: A `crate::` path in a test stays valid when the test file moves.

use syn::visit::Visit;
use syn::{Macro, Path as SynPath, UseTree, VisRestricted};

use crate::file::place::Place;
use crate::file::rs::macro_body::stmts;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "test-crate-path";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    if !Place::new(file.path, file.srcs).in_folder("tests") {
        return Vec::new();
    }
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
        if let Some(first) = path.segments.first()
            && first.ident == "super"
        {
            report_super(self.report, first.ident.span());
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_use_tree(&mut self, tree: &UseTree) {
        report_use(tree, self.report);
    }

    fn visit_vis_restricted(&mut self, _vis: &VisRestricted) {}

    fn visit_macro(&mut self, mac: &Macro) {
        syn::visit::visit_macro(self, mac);
        for stmt in &stmts(mac) {
            self.visit_stmt(stmt);
        }
    }
}

fn report_use(tree: &UseTree, report: &mut RuleReport<'_>) {
    match tree {
        UseTree::Path(path) if path.ident == "super" => report_super(report, path.ident.span()),
        UseTree::Name(name) if name.ident == "super" => report_super(report, name.ident.span()),
        UseTree::Rename(name) if name.ident == "super" => report_super(report, name.ident.span()),
        UseTree::Group(group) => {
            for item in &group.items {
                report_use(item, report);
            }
        }
        _ => {}
    }
}

fn report_super(report: &mut RuleReport<'_>, span: proc_macro2::Span) {
    report.at_span(
        span,
        "A test file uses a `super` path.",
        "Use a `crate::` path.",
    );
}
