// The page context does not import `PageContext` from `ian-framework-fe`.
// Import `PageContext` from `ian-framework-fe`.
// The page context has no exported class that extends `PageContext`.
// Write the page context as an exported class that extends `PageContext`.
// The page context has no constant `initialState`.
// Declare `const initialState` with the store type from `./types`.
// The type of `initialState` does not come from `./types`.
// Give `initialState` the store type from `./types`.
// Why a lint rule: Each page context has the same shape, and the store type of a page is in one file.

use oxc_ast::ast::{Class, Declaration, Statement, TSType};

use crate::file::place::Place;
use crate::file::ts::declaration::{Constant, constant, extends, imports};
use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "page-context";

const BASE: &str = "PageContext";
const PACKAGE: &str = "ian-framework-fe";
const INITIAL: &str = "initialState";
const TYPES: &str = "./types";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let place = Place::new(file.path, file.srcs);
    let ["page", _, "context.ts"] = place.parts()[..] else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    if !imports(program, PACKAGE)
        .iter()
        .any(|(imported, local)| *imported == BASE && *local == BASE)
    {
        report.at_offset(
            0,
            format!("The page context does not import `{BASE}` from `{PACKAGE}`."),
            format!("Import `{BASE}` from `{PACKAGE}`."),
        );
    }
    let class = program.body.iter().find_map(context_class);
    if class.is_none() {
        report.at_offset(
            0,
            format!("The page context has no exported class that extends `{BASE}`."),
            format!("Write the page context as an exported class that extends `{BASE}`."),
        );
    }
    match constant(program, INITIAL) {
        Constant::Absent if class.is_some_and(has_no_store) => {}
        Constant::Absent => report.at_offset(
            0,
            format!("The page context has no constant `{INITIAL}`."),
            format!("Declare `const {INITIAL}` with the store type from `{TYPES}`."),
        ),
        Constant::Untyped(offset) => report_type(&mut report, offset),
        Constant::Typed(offset, name) => {
            if !imports(program, TYPES)
                .iter()
                .any(|(_, local)| *local == name)
            {
                report_type(&mut report, offset);
            }
        }
    }
    report.finish()
}

fn context_class<'a, 'ast>(statement: &'a Statement<'ast>) -> Option<&'a Class<'ast>> {
    let Statement::ExportDeclaration(export) = statement else {
        return None;
    };
    match &export.declaration {
        Declaration::ClassDeclaration(class) if extends(class, BASE) => Some(class),
        _ => None,
    }
}

fn has_no_store(class: &Class<'_>) -> bool {
    class
        .heritage
        .as_ref()
        .and_then(|heritage| heritage.type_arguments.as_ref())
        .is_some_and(|arguments| {
            matches!(arguments.params.as_slice(), [TSType::TSObjectKeyword(_)])
        })
}

fn report_type(report: &mut RuleReport<'_>, offset: u32) {
    report.at_offset(
        offset,
        format!("The type of `{INITIAL}` does not come from `{TYPES}`."),
        format!("Give `{INITIAL}` the store type from `{TYPES}`."),
    );
}
