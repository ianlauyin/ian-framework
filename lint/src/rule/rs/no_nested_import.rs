// The braces of this `use` contain a path or braces.
// Write one `use` for each path. Put only names in the braces.
// Why a lint rule: A `use` with one path is easy to read, to sort, and to change.

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{ItemUse, UseTree};

use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "no-nested-import";

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
        if is_nested(&item.tree) {
            self.report.at_span(
                item.span(),
                "The braces of this `use` contain a path or braces.",
                "Write one `use` for each path. Put only names in the braces.",
            );
        }
    }
}

fn is_nested(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => is_nested(&path.tree),
        UseTree::Group(group) => group
            .items
            .iter()
            .any(|item| matches!(item, UseTree::Path(_) | UseTree::Group(_))),
        UseTree::Name(_) | UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}
