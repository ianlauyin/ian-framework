// `{name}` can stop the process.
// Return an error.
// Why a lint rule: A panic in request-time code stops the server.

use syn::visit::Visit;
use syn::{ExprMethodCall, ImplItemConst, ItemConst, Macro};

use crate::file::place::Place;
use crate::file::rs::macro_body::stmts;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "no-panic";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);
pub(crate) const FRAMEWORK_RS_RULE: RsRule = RsRule::file(check_framework);
const METHODS: &[&str] = &["unwrap", "expect"];
const MACROS: &[&str] = &["panic", "unreachable", "todo"];

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    check_file(file, true)
}

pub fn check_framework(file: &RsFile) -> Vec<Diagnostic> {
    check_file(file, false)
}

fn check_file(file: &RsFile, allow_main: bool) -> Vec<Diagnostic> {
    let place = Place::new(file.path, file.srcs);
    if place.in_folder("tests") || (allow_main && place.is_file("main.rs")) {
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
    fn visit_item_const(&mut self, _item: &ItemConst) {}

    fn visit_impl_item_const(&mut self, _item: &ImplItemConst) {}

    fn visit_expr_method_call(&mut self, call: &ExprMethodCall) {
        if METHODS.iter().any(|name| call.method == name) {
            self.report.at_span(
                call.method.span(),
                format!("`{}` can stop the process.", call.method),
                "Return an error.",
            );
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, mac: &Macro) {
        if let Some(last) = mac.path.segments.last()
            && MACROS.iter().any(|name| last.ident == name)
        {
            self.report.at_span(
                last.ident.span(),
                format!("`{}!` can stop the process.", last.ident),
                "Return an error.",
            );
        }
        syn::visit::visit_macro(self, mac);
        for stmt in &stmts(mac) {
            self.visit_stmt(stmt);
        }
    }
}
