// The type `{name}` is an object shape.
// Declare the object shape with `interface`.
// Why a lint rule: An object shape has one form of declaration.

use oxc_ast::ast::{TSType, TSTypeAliasDeclaration};
use oxc_ast_visit::Visit;

use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "interface-object-shape";

pub(crate) const TS_RULE: TsRule = TsRule::file(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut aliases = Aliases {
        report: &mut report,
    };
    aliases.visit_program(program);
    report.finish()
}

struct Aliases<'a, 'b> {
    report: &'b mut RuleReport<'a>,
}

impl<'ast> Visit<'ast> for Aliases<'_, '_> {
    fn visit_ts_type_alias_declaration(&mut self, it: &TSTypeAliasDeclaration<'ast>) {
        if matches!(it.type_annotation, TSType::TSTypeLiteral(_)) {
            self.report.at_offset(
                it.span.start,
                format!("The type `{}` is an object shape.", it.id.name),
                "Declare the object shape with `interface`.",
            );
        }
    }
}
