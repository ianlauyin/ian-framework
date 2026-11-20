// The file has a comment.
// Put the decision in the catalog. Or add a test that prevents that reason.
// Why a lint rule: A comment hides a decision that belongs in the catalog.

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, strip_shebang, tokenize};

use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::Report;
use crate::rule::rs::file::RsFile;

const RULE: &str = "no-comments";

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let mut report = Report::new(file.path, file.source, RULE);
    let skipped = strip_shebang(file.source).unwrap_or(0);
    let mut offset = skipped as u32;
    for token in tokenize(&file.source[skipped..], FrontmatterAllowed::Yes) {
        if matches!(
            token.kind,
            TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
        ) {
            report.at_offset(
                offset,
                "The file has a comment.",
                "Put the decision in the catalog. Or add a test that prevents that reason.",
            );
        }
        offset += token.len;
    }
    report.finish()
}
