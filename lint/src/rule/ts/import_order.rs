// This import is out of import order.
// Put the other packages first, then `solid-js` and `@solidjs`, then `ian-framework-fe`, then the relative paths, then the imports with no names.
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
const SOLID: u8 = 1;
const FRAMEWORK: u8 = 2;
const RELATIVE: u8 = 3;
const NO_NAMES: u8 = 4;
const FRAMEWORK_PACKAGE: &str = "ian-framework-fe";
const FIX: &str = "Put the other packages first, then `solid-js` and `@solidjs`, then `ian-framework-fe`, then the relative paths, then the imports with no names.";

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut last_import: Option<u8> = None;
    let mut last_reexport: Option<u8> = None;
    for statement in &program.body {
        let (span, source, no_names, last) = match statement {
            Statement::ImportDeclaration(declaration) => (
                declaration.span,
                &declaration.source,
                declaration
                    .specifiers
                    .as_ref()
                    .is_none_or(|specifiers| specifiers.is_empty()),
                &mut last_import,
            ),
            Statement::ExportFromDeclaration(declaration) => (
                declaration.span,
                &declaration.source,
                false,
                &mut last_reexport,
            ),
            Statement::ExportAllDeclaration(declaration) => (
                declaration.span,
                &declaration.source,
                false,
                &mut last_reexport,
            ),
            _ => continue,
        };
        check_import(span, source, no_names, last, &mut report);
    }
    report.finish()
}

fn check_import(
    span: Span,
    source: &StringLiteral<'_>,
    no_names: bool,
    last: &mut Option<u8>,
    report: &mut RuleReport<'_>,
) {
    let Some(text) = source.value.as_str() else {
        return;
    };
    let group = if no_names { NO_NAMES } else { group(text) };
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
    } else if in_package(specifier, FRAMEWORK_PACKAGE) {
        FRAMEWORK
    } else if in_package(specifier, "solid-js") || specifier.starts_with("@solidjs/") {
        SOLID
    } else {
        OTHER
    }
}

fn in_package(specifier: &str, package: &str) -> bool {
    specifier
        .strip_prefix(package)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}
