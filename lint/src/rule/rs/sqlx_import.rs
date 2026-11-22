// `sqlx` is used outside the `db` folder.
// Move the code to the `db` folder.
// Why a lint rule: Only the `db` folder reads and writes the database.

use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{Attribute, Macro, Path as SynPath, Token, UseTree};

use crate::file::rs::macro_body::stmts;
use crate::file::rs::place::Place;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "sqlx-import";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let place = Place::new(file.path, file.srcs);
    if place.in_folder("db") || place.in_folder("tests") {
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
        if path.segments.len() > 1 && path.segments[0].ident == "sqlx" {
            report_sqlx(self.report, path.segments[0].ident.span());
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_use_tree(&mut self, tree: &UseTree) {
        report_use(tree, self.report);
    }

    fn visit_attribute(&mut self, attr: &Attribute) {
        if attr.path().is_ident("derive") {
            let paths = attr.parse_args_with(Punctuated::<SynPath, Token![,]>::parse_terminated);
            for path in paths.iter().flatten() {
                self.visit_path(path);
            }
        }
        syn::visit::visit_attribute(self, attr);
    }

    fn visit_macro(&mut self, mac: &Macro) {
        syn::visit::visit_macro(self, mac);
        for stmt in &stmts(mac) {
            self.visit_stmt(stmt);
        }
    }
}

fn report_use(tree: &UseTree, report: &mut RuleReport<'_>) {
    let ident = match tree {
        UseTree::Path(path) => &path.ident,
        UseTree::Name(name) => &name.ident,
        UseTree::Rename(name) => &name.ident,
        UseTree::Group(group) => {
            for item in &group.items {
                report_use(item, report);
            }
            return;
        }
        UseTree::Glob(_) => return,
    };
    if ident == "sqlx" {
        report_sqlx(report, ident.span());
    }
}

fn report_sqlx(report: &mut RuleReport<'_>, span: Span) {
    report.at_span(
        span,
        "`sqlx` is used outside the `db` folder.",
        "Move the code to the `db` folder.",
    );
}
