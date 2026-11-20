// `Exception::__new` builds an `Exception` outside `exception.rs`.
// Build the `Exception` with `exception!`.
// Why a lint rule: The `exception!` macro builds an `Exception`.

use syn::Path as SynPath;
use syn::spanned::Spanned;
use syn::visit::Visit;

use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::place::Place;
use crate::rule::rs::rule::RsRule;
use crate::rule::rs::source::RsFile;

const RULE: &str = "exception-new";

pub(crate) const RS_RULE: RsRule = RsRule::file(check, None);

pub(crate) fn check(file: &RsFile) -> Vec<Diagnostic> {
    if Place::new(file.path, file.srcs).is_file("exception.rs") {
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
        let mut previous = String::new();
        for segment in &path.segments {
            let name = segment.ident.to_string();
            if previous == "Exception" && name == "__new" {
                self.report.at_span(
                    path.span(),
                    "`Exception::__new` builds an `Exception` outside `exception.rs`.",
                    "Build the `Exception` with `exception!`.",
                );
                break;
            }
            previous = name;
        }
        syn::visit::visit_path(self, path);
    }
}
