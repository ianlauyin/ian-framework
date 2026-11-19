// The file has a comment.
// Put the decision in the catalog. Or add a test that prevents that reason.
// Why a lint rule: A comment hides a decision that belongs in the catalog.

use std::fs;
use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "no-comments";

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    check_source(path, &source)
}

pub(crate) fn check_source(path: &Path, source: &str) -> Vec<Diagnostic> {
    Finder::typescript_source(path, source, RULE, |program, finder| {
        for comment in &program.comments {
            finder.report(
                comment.span.start,
                "The file has a comment.",
                "Put the decision in the catalog. Or add a test that prevents that reason.",
            );
        }
    })
}
