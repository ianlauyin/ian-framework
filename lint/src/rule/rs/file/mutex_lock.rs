// `lock().unwrap` locks a standard mutex.
// `lock().unwrap_or` locks a standard mutex.
// Lock it with `mutex::lock`.
// Why a lint rule: `mutex::lock` clears a poisoned mutex. `lock().await` stays valid.

use syn::visit::Visit;
use syn::{Expr, ExprMethodCall};

use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::place::Place;
use crate::rule::rs::rule::RsRule;
use crate::rule::rs::source::RsFile;

const RULE: &str = "mutex-lock";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub(crate) fn check(file: &RsFile) -> Vec<Diagnostic> {
    if Place::new(file.path, file.srcs).is_file("mutex.rs") {
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
    fn visit_expr_method_call(&mut self, call: &ExprMethodCall) {
        if (call.method == "unwrap" || call.method == "unwrap_or")
            && let Expr::MethodCall(inner) = &*call.receiver
            && inner.method == "lock"
        {
            let method = call.method.to_string();
            self.report.at_span(
                call.method.span(),
                format!("`lock().{method}` locks a standard mutex."),
                "Lock it with `mutex::lock`.",
            );
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}
