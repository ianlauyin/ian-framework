// The type `{name}` is in the `index.ts` of a module.
// Move the type to `types.ts`.
// The module has no exported class `{Module}Module` that extends `Module`.
// Write the module as `export class {Module}Module extends Module<{Module}Store>`.
// The last statement does not export the instance `{module}Module`.
// End the file with `export const {module}Module = new {Module}Module();`.
// Why a lint rule: Each module has the same shape, and other files use only the instance.

use oxc_ast::ast::{BindingPattern, Declaration, Expression, Statement};
use oxc_span::GetSpan;

use crate::file::place::Place;
use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "module-shape";

const BASE: &str = "Module";

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
    let instance = format!("{}{BASE}", camel_case(folder));
    let mut report = RuleReport::new(file.path, file.source, RULE);
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
    if !program
        .body
        .last()
        .is_some_and(|statement| is_instance(statement, &class, &instance))
    {
        report.at_offset(
            program.body.last().map_or(0, |last| last.span().start),
            format!("The last statement does not export the instance `{instance}`."),
            format!("End the file with `export const {instance} = new {class}();`."),
        );
    }
    report.finish()
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
    let extends = match class
        .heritage
        .as_ref()
        .map(|heritage| heritage.expression.without_parentheses())
    {
        Some(Expression::Identifier(base)) => base.name.as_str() == BASE,
        _ => false,
    };
    extends && class.id.as_ref().is_some_and(|id| id.name.as_str() == name)
}

fn is_instance(statement: &Statement<'_>, class: &str, instance: &str) -> bool {
    let Statement::ExportDeclaration(export) = statement else {
        return false;
    };
    let Declaration::VariableDeclaration(variable) = &export.declaration else {
        return false;
    };
    let [declarator] = variable.declarations.as_slice() else {
        return false;
    };
    let BindingPattern::BindingIdentifier(id) = &declarator.id else {
        return false;
    };
    let Some(Expression::NewExpression(new)) = &declarator.init else {
        return false;
    };
    let Expression::Identifier(callee) = &new.callee else {
        return false;
    };
    id.name.as_str() == instance && callee.name.as_str() == class
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

fn camel_case(name: &str) -> String {
    let pascal = pascal_case(name);
    let mut chars = pascal.chars();
    let Some(first) = chars.next() else {
        return pascal;
    };
    first.to_lowercase().chain(chars).collect()
}
