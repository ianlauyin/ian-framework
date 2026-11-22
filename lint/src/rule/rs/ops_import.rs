// A `crate::ops` path is outside the `ops` module.
// Move the code into the `ops` module, or move the shared item out of it.
// Why a lint rule: Only the ops router uses the code in the `ops` module.

use syn::visit::Visit;
use syn::{Ident, Macro, Path as SynPath, UseTree};

use crate::file::place::Place;
use crate::file::rs::macro_body::stmts;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "ops-import";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let place = Place::new(file.path, file.srcs);
    if place.in_folder("ops")
        || place.is_file("ops.rs")
        || place.is_file("main.rs")
        || place.in_folder("tests")
    {
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
        let mut segments = path.segments.iter();
        if let (Some(first), Some(second)) = (segments.next(), segments.next()) {
            report_pair(&first.ident, &second.ident, self.report);
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_use_tree(&mut self, tree: &UseTree) {
        if let UseTree::Path(path) = tree
            && path.ident == "crate"
        {
            report_use(&path.ident, &path.tree, self.report);
        }
    }

    fn visit_macro(&mut self, mac: &Macro) {
        syn::visit::visit_macro(self, mac);
        for stmt in &stmts(mac) {
            self.visit_stmt(stmt);
        }
    }
}

fn report_use(first: &Ident, tree: &UseTree, report: &mut RuleReport<'_>) {
    match tree {
        UseTree::Path(path) => report_pair(first, &path.ident, report),
        UseTree::Name(name) => report_pair(first, &name.ident, report),
        UseTree::Rename(name) => report_pair(first, &name.ident, report),
        UseTree::Group(group) => {
            for item in &group.items {
                report_use(first, item, report);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn report_pair(first: &Ident, second: &Ident, report: &mut RuleReport<'_>) {
    if first != "crate" || second != "ops" {
        return;
    }
    report.at_span(
        second.span(),
        "A `crate::ops` path is outside the `ops` module.",
        "Move the code into the `ops` module, or move the shared item out of it.",
    );
}
