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
    Finder::rust_source(path, source, RULE, |file, finder| {
        let mut pass = Pass { finder };
        pass.visit_file(file);
    })
}

struct Pass<'a, 'b> {
    finder: &'b mut Finder<'a>,
}

impl Visit<'_> for Pass<'_, '_> {
    fn visit_expr_method_call(&mut self, call: &ExprMethodCall) {
        if (call.method == "unwrap" || call.method == "unwrap_or")
            && let Expr::MethodCall(inner) = &*call.receiver
            && inner.method == "lock"
        {
            let method = call.method.to_string();
            self.finder.at_span(
                call.method.span(),
                format!("`lock().{method}` locks a standard mutex."),
                "Lock it with `mutex::lock`.",
            );
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}
