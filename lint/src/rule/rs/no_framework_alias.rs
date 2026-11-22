// This `use` gives an alias to an `ian_framework_rs` item.
// Import the item with its name.
// Why a lint rule: A framework item has the same name in each file.

use syn::visit::Visit;
use syn::{ItemUse, UseTree};

use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "no-framework-alias";

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
        if let UseTree::Path(path) = &item.tree
            && path.ident == "ian_framework_rs"
        {
            report_use(&path.tree, self.report);
        }
    }
}

fn report_use(tree: &UseTree, report: &mut RuleReport<'_>) {
    match tree {
        UseTree::Path(path) => report_use(&path.tree, report),
        UseTree::Group(group) => {
            for item in &group.items {
                report_use(item, report);
            }
        }
        UseTree::Rename(name) if name.rename != "_" => {
            report.at_span(
                name.rename.span(),
                "This `use` gives an alias to an `ian_framework_rs` item.",
                "Import the item with its name.",
            );
        }
        _ => {}
    }
}
