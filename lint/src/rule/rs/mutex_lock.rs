// `lock().unwrap` locks a standard mutex.
// `lock().unwrap_or` locks a standard mutex.
// `lock().expect` locks a standard mutex.
// Lock it with `mutex::lock`.
// Why a lint rule: `mutex::lock` clears a poisoned mutex. `lock().await` stays valid.

use syn::visit::Visit;
use syn::{Expr, ExprMethodCall, Macro};

use crate::file::place::Place;
use crate::file::rs::macro_body::stmts;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "mutex-lock";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);
const METHODS: &[&str] = &["unwrap", "unwrap_or", "expect"];

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let place = Place::new(file.path, file.srcs);
    if place.is_file("mutex.rs") || place.in_folder("tests") {
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
        if METHODS.iter().any(|name| call.method == name)
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

    fn visit_macro(&mut self, mac: &Macro) {
        syn::visit::visit_macro(self, mac);
        for stmt in &stmts(mac) {
            self.visit_stmt(stmt);
        }
    }
}
