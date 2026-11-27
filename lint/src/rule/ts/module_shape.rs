// The type `{name}` is in the `index.ts` of a module.
// Move the type to `types.ts`.
// The module has no exported class `{Module}Module` that extends `Module`.
// Write the module as `export class {Module}Module extends Module<{Module}Store>`.
// The module does not import `Module` from `ian-framework-fe`.
// Import `Module` from `ian-framework-fe`.
// The module has no constant `initialStore`.
// Declare `const initialStore` with the store type from `./types`.
// The type of `initialStore` does not come from `./types`.
// Give `initialStore` the store type from `./types`.
// Why a lint rule: Each module has the same shape, and only a page gets the module, through `this.module`.

use oxc_ast::ast::{Declaration, Statement};
use oxc_span::GetSpan;

use crate::file::place::Place;
use crate::file::ts::declaration::{Constant, constant, extends, imports};
use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "module-shape";

const BASE: &str = "Module";
const PACKAGE: &str = "ian-framework-fe";
const INITIAL: &str = "initialStore";
const TYPES: &str = "./types";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let place = Place::new(file.path, file.srcs);
    let ["module", folder, "index.ts"] = place.parts()[..] else {
        return Vec::new();
    };
    let class = format!("{}{BASE}", pascal_case(folder));
    let mut report = RuleReport::new(file.path, file.source, RULE);
    if !imports(program, PACKAGE)
        .iter()
        .any(|(imported, local)| *imported == BASE && *local == BASE)
    {
        report.at_offset(
            0,
            format!("The module does not import `{BASE}` from `{PACKAGE}`."),
            format!("Import `{BASE}` from `{PACKAGE}`."),
        );
    }
    match constant(program, INITIAL) {
        Constant::Absent => report.at_offset(
            0,
            format!("The module has no constant `{INITIAL}`."),
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
    let mut has_class = false;
    for statement in &program.body {
        if let Some(name) = type_name(statement) {
            report.at_offset(
                statement.span().start,
                format!("The type `{name}` is in the `index.ts` of a module."),
                "Move the type to `types.ts`.",
            );
        }
        has_class |= is_module_class(statement, &class);
    }
    if !has_class {
        report.at_offset(
            0,
            format!("The module has no exported class `{class}` that extends `{BASE}`."),
            format!(
                "Write the module as `export class {class} extends {BASE}<{}Store>`.",
                pascal_case(folder)
            ),
        );
    }
    report.finish()
}

fn report_type(report: &mut RuleReport<'_>, offset: u32) {
    report.at_offset(
        offset,
        format!("The type of `{INITIAL}` does not come from `{TYPES}`."),
        format!("Give `{INITIAL}` the store type from `{TYPES}`."),
    );
}

fn type_name<'a>(statement: &'a Statement<'_>) -> Option<&'a str> {
    match statement {
        Statement::TSTypeAliasDeclaration(item) => Some(item.id.name.as_str()),
        Statement::TSInterfaceDeclaration(item) => Some(item.id.name.as_str()),
        Statement::ExportDeclaration(export) => match &export.declaration {
            Declaration::TSTypeAliasDeclaration(item) => Some(item.id.name.as_str()),
            Declaration::TSInterfaceDeclaration(item) => Some(item.id.name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn is_module_class(statement: &Statement<'_>, name: &str) -> bool {
    let Statement::ExportDeclaration(export) = statement else {
        return false;
    };
    let Declaration::ClassDeclaration(class) = &export.declaration else {
        return false;
    };
    extends(class, BASE) && class.id.as_ref().is_some_and(|id| id.name.as_str() == name)
}

fn pascal_case(name: &str) -> String {
    let mut out = String::new();
    for word in name.split(['-', '_']) {
        let mut chars = word.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
    out
}
