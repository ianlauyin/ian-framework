// The file has a comment.
// Put the decision in the catalog. Or add a test that prevents that reason.
// Why a lint rule: A comment hides a decision that belongs in the catalog.

use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::ts::file::TsFile;
use crate::rule::report::Report;

const RULE: &str = "no-comments";

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = Report::new(file.path, file.source, RULE);
    for comment in &program.comments {
        report.at_offset(
            comment.span.start,
            "The file has a comment.",
            "Put the decision in the catalog. Or add a test that prevents that reason.",
        );
    }
    report.finish()
}
