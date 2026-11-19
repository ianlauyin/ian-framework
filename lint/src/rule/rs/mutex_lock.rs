// `lock().unwrap` locks a standard mutex.
// `lock().unwrap_or` locks a standard mutex.
// Lock it with `mutex::lock`.
// Why a lint rule: `mutex::lock` clears a poisoned mutex. `lock().await` stays valid.

use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::Visit;
use syn::{Expr, ExprMethodCall};

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;
use crate::rule::rs::place::Place;

const RULE: &str = "mutex-lock";

pub fn check(path: &Path, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    check_source(path, &source, srcs)
}

pub(crate) fn check_source(path: &Path, source: &str, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    if Place::new(path, srcs).is_file("mutex.rs") {
        return Vec::new();
    }
    let Ok(file) = syn::parse_file(source) else {
        return Vec::new();
    };
    let mut pass = Pass {
        finder: Finder::new(path, source),
    };
    pass.visit_file(&file);
    pass.finder.finish()
}

struct Pass<'a> {
    finder: Finder<'a>,
}

impl Visit<'_> for Pass<'_> {
    fn visit_expr_method_call(&mut self, call: &ExprMethodCall) {
        if (call.method == "unwrap" || call.method == "unwrap_or")
            && let Expr::MethodCall(inner) = &*call.receiver
            && inner.method == "lock"
        {
            let method = call.method.to_string();
            self.finder.report(
                call.method.span().byte_range().start as u32,
                RULE,
                format!("`lock().{method}` locks a standard mutex."),
                "Lock it with `mutex::lock`.",
            );
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}
