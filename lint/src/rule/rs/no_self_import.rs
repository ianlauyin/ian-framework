// The braces of this `use` contain `self`.
// Import only the module.
// Why a lint rule: One `use` imports one thing. The module path shows the source of each item.

use syn::visit::Visit;
use syn::{ItemUse, UseTree};

use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "no-self-import";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
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
    fn visit_item_use(&mut self, item: &ItemUse) {
        report_use(&item.tree, false, self.report);
    }
}

fn report_use(tree: &UseTree, in_braces: bool, report: &mut RuleReport<'_>) {
    match tree {
        UseTree::Path(path) => report_use(&path.tree, in_braces, report),
        UseTree::Group(group) => {
            for item in &group.items {
                report_use(item, true, report);
            }
        }
        UseTree::Name(name) if in_braces && name.ident == "self" => {
            report_self(report, name.ident.span());
        }
        UseTree::Rename(name) if in_braces && name.ident == "self" => {
            report_self(report, name.ident.span());
        }
        _ => {}
    }
}

fn report_self(report: &mut RuleReport<'_>, span: proc_macro2::Span) {
    report.at_span(
        span,
        "The braces of this `use` contain `self`.",
        "Import only the module.",
    );
}
