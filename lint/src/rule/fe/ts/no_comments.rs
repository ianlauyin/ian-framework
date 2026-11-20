// The file has a comment.
// Put the decision in the catalog. Or add a test that prevents that reason.
// Why a lint rule: A comment hides a decision that belongs in the catalog.

use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::ts::file::TsFile;
use crate::rule::fe::ts::rule::TsRule;
use crate::rule::report::RuleReport;

const RULE: &str = "no-comments";

pub(crate) const TS_RULE: TsRule = TsRule::file(check, None);

pub(crate) fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    for comment in &program.comments {
        report.at_offset(
            comment.span.start,
            "The file has a comment.",
            "Put the decision in the catalog. Or add a test that prevents that reason.",
        );
    }
    report.finish()
}
