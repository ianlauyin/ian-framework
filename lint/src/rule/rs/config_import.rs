// `{name}` is used outside `config.rs`.
// Read the value in `config.rs` and pass the value to the caller.
// Why a lint rule: Only `config.rs` reads the settings of the app.

use syn::visit::Visit;
use syn::{Ident, Macro, Path as SynPath, UseTree};

use crate::file::place::Place;
use crate::file::rs::macro_body::stmts;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "config-import";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);
const FRAMEWORK: &str = "ian_framework_rs";
const SECRETS: &[&str] = &["secret_manager", "SecretManager"];

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let place = Place::new(file.path, file.srcs);
    if place.is_file("config.rs") || place.is_file("log_config.rs") || place.in_folder("tests") {
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
        UseTree::Glob(_) => report_path(&idents, report),
    }
}

fn report_path(idents: &[&Ident], report: &mut RuleReport<'_>) {
    let Some((name, ident)) = restricted(idents) else {
        return;
    };
    report.at_span(
        ident.span(),
        format!("`{name}` is used outside `config.rs`."),
        "Read the value in `config.rs` and pass the value to the caller.",
    );
}

fn restricted<'a>(idents: &[&'a Ident]) -> Option<(String, &'a Ident)> {
    if let [first, second, ..] = idents
        && *first == FRAMEWORK
        && *second == "env"
    {
        return Some((format!("{FRAMEWORK}::env"), second));
    }
    idents
        .windows(2)
        .find(|pair| pair[0] == "gcp" && SECRETS.iter().any(|name| pair[1] == name))
        .map(|pair| (pair[1].to_string(), pair[1]))
}
