// `std::env` is used.
// Read the value in `config.rs` with `ian_framework_rs::env` and pass the value to the caller.
// Why a lint rule: The app reads its settings only through the framework.

use syn::visit::Visit;
use syn::{Ident, Macro, Path as SynPath, UseTree};

use crate::file::place::Place;
use crate::file::rs::macro_body::stmts;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "std-env";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    if Place::new(file.path, file.srcs).in_folder("tests") {
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
        let idents: Vec<_> = path.segments.iter().map(|segment| &segment.ident).collect();
        report_path(&idents, self.report);
        syn::visit::visit_path(self, path);
    }

    fn visit_use_tree(&mut self, tree: &UseTree) {
        report_use(tree, &[], self.report);
    }

    fn visit_macro(&mut self, mac: &Macro) {
        syn::visit::visit_macro(self, mac);
        for stmt in &stmts(mac) {
            self.visit_stmt(stmt);
        }
    }
}

fn report_use<'a>(tree: &'a UseTree, prefix: &[&'a Ident], report: &mut RuleReport<'_>) {
    let mut idents = prefix.to_vec();
    match tree {
        UseTree::Path(path) => {
            idents.push(&path.ident);
            if is_std_env(&idents) {
                report_path(&idents, report);
                return;
            }
            report_use(&path.tree, &idents, report);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                report_use(item, prefix, report);
            }
        }
        UseTree::Name(name) => {
            idents.push(&name.ident);
            report_path(&idents, report);
        }
        UseTree::Rename(name) => {
            idents.push(&name.ident);
            report_path(&idents, report);
        }
        UseTree::Glob(_) => {}
    }
}

fn report_path(idents: &[&Ident], report: &mut RuleReport<'_>) {
    if !is_std_env(idents) {
        return;
    }
    report.at_span(
        idents[1].span(),
        "`std::env` is used.",
        "Read the value in `config.rs` with `ian_framework_rs::env` and pass the value to the caller.",
    );
}

fn is_std_env(idents: &[&Ident]) -> bool {
    matches!(idents, [first, second, ..] if *first == "std" && *second == "env")
}
