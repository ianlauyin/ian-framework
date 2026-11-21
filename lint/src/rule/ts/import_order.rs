// This import is out of import order.
// Put the other packages first, then `ian-framework-fe`, then the relative paths.
// Why a lint rule: That order keeps the imports from each source in one place.

use oxc_ast::ast::{Statement, StringLiteral};
use oxc_span::Span;

use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "import-order";

pub(crate) const TS_RULE: TsRule = TsRule::file(check);

const OTHER: u8 = 0;
const FRAMEWORK: u8 = 1;
const RELATIVE: u8 = 2;
const FRAMEWORK_PACKAGE: &str = "ian-framework-fe";
const FIX: &str = "Put the other packages first, then `ian-framework-fe`, then the relative paths.";

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut last_import: Option<u8> = None;
    let mut last_reexport: Option<u8> = None;
    for statement in &program.body {
        let (span, source, last) = match statement {
            Statement::ImportDeclaration(declaration) if declaration.specifiers.is_some() => {
                (declaration.span, &declaration.source, &mut last_import)
            }
            Statement::ExportFromDeclaration(declaration) => {
                (declaration.span, &declaration.source, &mut last_reexport)
            }
            Statement::ExportAllDeclaration(declaration) => {
                (declaration.span, &declaration.source, &mut last_reexport)
            }
            _ => continue,
        };
        check_import(span, source, last, &mut report);
    }
    report.finish()
}

fn check_import(
    span: Span,
    source: &StringLiteral<'_>,
    last: &mut Option<u8>,
    report: &mut RuleReport<'_>,
) {
    let Some(text) = source.value.as_str() else {
        return;
    };
    let group = group(text);
    if last.is_some_and(|previous| group < previous) {
        report.at_offset(span.start, "This import is out of import order.", FIX);
    }
    *last = Some(last.map_or(group, |previous| previous.max(group)));
}

fn group(specifier: &str) -> u8 {
    if specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
    {
        RELATIVE
    } else if specifier == FRAMEWORK_PACKAGE
        || specifier
            .strip_prefix(FRAMEWORK_PACKAGE)
            .is_some_and(|rest| rest.starts_with('/'))
    {
        FRAMEWORK
    } else {
        OTHER
    }
}
