// The file has a comment.
// Put the decision in the catalog. Or add a test that prevents that reason.
// Why a lint rule: A comment hides a decision that belongs in the catalog.

use std::fs;
use std::path::Path;

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, strip_shebang, tokenize};

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "no-comments";

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut finder = Finder::new(path, &source);
    let skipped = strip_shebang(&source).unwrap_or(0);
    let mut offset = skipped as u32;
    for token in tokenize(&source[skipped..], FrontmatterAllowed::Yes) {
        if matches!(
            token.kind,
            TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
        ) {
            finder.report(
                offset,
                RULE,
                "The file has a comment.",
                "Put the decision in the catalog. Or add a test that prevents that reason.",
            );
        }
        offset += token.len;
    }
    finder.finish()
}
