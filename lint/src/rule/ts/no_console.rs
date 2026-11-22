// `console` is used.
// Write the event with `Logger`.
// Why a lint rule: An event that goes to `console` does not go to the log server.

use oxc_ast::ast::IdentifierReference;
use oxc_ast_visit::Visit;

use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "no-console";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut names = Names {
        report: &mut report,
    };
    names.visit_program(program);
    report.finish()
}

struct Names<'a, 'b> {
    report: &'b mut RuleReport<'a>,
}

impl<'ast> Visit<'ast> for Names<'_, '_> {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'ast>) {
        if it.name.as_str() == "console" {
            self.report.at_offset(
                it.span.start,
                "`console` is used.",
                "Write the event with `Logger`.",
            );
        }
    }
}
