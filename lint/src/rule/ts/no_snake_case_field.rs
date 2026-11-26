// The field `{name}` is snake_case.
// Rename the field in camelCase.
// Why a lint rule: The JSON of the API is camelCase, so a snake_case field in a type is a field the server does not send.

use oxc_ast::ast::TSPropertySignature;
use oxc_ast_visit::Visit;

use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "no-snake-case-field";

pub(crate) const TS_RULE: TsRule = TsRule::file(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut fields = Fields {
        report: &mut report,
    };
    fields.visit_program(program);
    report.finish()
}

struct Fields<'a, 'b> {
    report: &'b mut RuleReport<'a>,
}

impl<'ast> Visit<'ast> for Fields<'_, '_> {
    fn visit_ts_property_signature(&mut self, it: &TSPropertySignature<'ast>) {
        if let Some(name) = it.key.static_name().map(|name| name.display().to_string())
            && name.trim_matches('_').contains('_')
        {
            self.report.at_offset(
                it.span.start,
                format!("The field `{name}` is snake_case."),
                "Rename the field in camelCase.",
            );
        }
        oxc_ast_visit::walk::walk_ts_property_signature(self, it);
    }
}
