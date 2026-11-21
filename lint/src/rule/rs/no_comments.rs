// The file has a comment.
// Put the decision in the catalog. Or add a test that prevents that reason.
// Why a lint rule: A comment hides a decision that belongs in the catalog.

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, strip_shebang, tokenize};

use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "no-comments";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let mut report = RuleReport::new(file.path, file.source, RULE);
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
