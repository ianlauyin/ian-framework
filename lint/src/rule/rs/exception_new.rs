// `Exception::__new` builds an `Exception` outside `exception.rs`.
// Build the `Exception` with `exception!`.
// Why a lint rule: The `exception!` macro builds an `Exception`.

use std::fs;
use std::path::{Path, PathBuf};

use syn::Path as SynPath;
use syn::spanned::Spanned;
use syn::visit::Visit;

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;
use crate::rule::rs::place::Place;

const RULE: &str = "exception-new";

pub fn check(path: &Path, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    check_source(path, &source, srcs)
}

pub(crate) fn check_source(path: &Path, source: &str, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    if Place::new(path, srcs).is_file("exception.rs") {
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
    fn visit_path(&mut self, path: &SynPath) {
        let mut previous = String::new();
        for segment in &path.segments {
            let name = segment.ident.to_string();
            if previous == "Exception" && name == "__new" {
                self.finder.report(
                    path.span().byte_range().start as u32,
                    RULE,
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
