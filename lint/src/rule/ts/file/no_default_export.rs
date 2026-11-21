// The file has a default export.
// Use a named export.
// Why a lint rule: A named export keeps one name for the item.

use oxc_ast::ast::{ModuleExportName, Statement};

use crate::parser::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::rule::TsRule;

const RULE: &str = "no-default-export";

pub(crate) const TS_RULE: TsRule = TsRule::file(check);

pub(crate) fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    for statement in &program.body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                report_default(&mut report, declaration.span.start);
            }
            Statement::ExportNamedDeclaration(declaration) => {
                report_specifiers(&mut report, &declaration.specifiers);
            }
            Statement::ExportFromDeclaration(declaration) => {
                report_specifiers(&mut report, &declaration.specifiers);
            }
            _ => {}
        }
    }
    report.finish()
}

fn report_specifiers(
    report: &mut RuleReport<'_>,
    specifiers: &[oxc_ast::ast::ExportSpecifier<'_>],
) {
    for specifier in specifiers {
        if is_default_name(&specifier.exported) {
            report_default(report, specifier.span.start);
        }
    }
}

fn is_default_name(name: &ModuleExportName<'_>) -> bool {
    match name {
        ModuleExportName::IdentifierName(name) => name.name.as_str() == "default",
        ModuleExportName::IdentifierReference(name) => name.name.as_str() == "default",
        ModuleExportName::StringLiteral(name) => name.value.as_str() == Some("default"),
    }
}

fn report_default(report: &mut RuleReport<'_>, offset: u32) {
    report.at_offset(
        offset,
        "The file has a default export.",
        "Use a named export.",
    );
}
