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
// The `index.ts` of a module has code that is not `initialStore` or the module class.
// Keep only `initialStore` and the module class in the file.
// The module index import `{specifier}` is not `registerModule` or a module class.
// Import only `registerModule` from `ian-framework-fe` and one module class from each module folder.
// The module index does not import `registerModule` from `ian-framework-fe`.
// Import `registerModule` from `ian-framework-fe`.
// The module index does not declare `ModuleMap` in `ian-framework-fe`.
// Declare module `ian-framework-fe` with interface `ModuleMap`.
// `module/index.ts` has code outside the module registration shape.
// Keep only module imports, the `ModuleMap` declaration, and the `registerModule` calls.
// Why a lint rule: Each module has the same shape, and only a page gets the module, through `this.module`.

use oxc_ast::ast::{
    BindingPattern, Declaration, Expression, ImportDeclaration, ImportDeclarationSpecifier,
    ModuleExportName, Statement, VariableDeclaration, VariableDeclarationKind,
};
use oxc_span::GetSpan;

use crate::file::place::Place;
use crate::file::ts::declaration::{Constant, constant, extends, imports};
use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "module-shape";

const BASE: &str = "Module";
const REGISTER: &str = "registerModule";
const MODULE_MAP: &str = "ModuleMap";
const PACKAGE: &str = "ian-framework-fe";
const INITIAL: &str = "initialStore";
const TYPES: &str = "./types";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let place = Place::new(file.path, file.srcs);
    match place.parts().as_slice() {
        ["module", "index.ts"] => check_module_index(file, program),
        ["module", folder, "index.ts"] => check_module(file, program, folder),
        _ => Vec::new(),
    }
}

fn check_module(
    file: &TsFile,
    program: &oxc_ast::ast::Program<'_>,
    folder: &str,
) -> Vec<Diagnostic> {
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
    let has_class = program
        .body
        .iter()
        .any(|statement| is_module_class(statement, &class));
    for statement in &program.body {
        if matches!(statement, Statement::ImportDeclaration(_)) {
            continue;
        }
        if let Some(name) = type_name(statement) {
            report.at_offset(
                statement.span().start,
                format!("The type `{name}` is in the `index.ts` of a module."),
                "Move the type to `types.ts`.",
            );
            continue;
        }
        if is_module_class(statement, &class) || is_initial_store(statement) {
            continue;
        }
        if !has_class && is_class(statement) {
            continue;
        }
        report.at_offset(
            statement.span().start,
            format!(
                "The `index.ts` of a module has code that is not `{INITIAL}` or the module class."
            ),
            format!("Keep only `{INITIAL}` and the module class in the file."),
        );
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

fn check_module_index(file: &TsFile, program: &oxc_ast::ast::Program<'_>) -> Vec<Diagnostic> {
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut has_module_map = false;
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(import) => check_index_import(&mut report, import),
            Statement::ExpressionStatement(expression) if is_register_call(expression) => {}
            Statement::TSExternalModuleDeclaration(module) if is_module_map(module) => {
                has_module_map = true;
            }
            _ => report_index_code(&mut report, statement.span().start),
        }
    }
    if !imports(program, PACKAGE)
        .iter()
        .any(|(imported, _)| *imported == REGISTER)
    {
        report.at_offset(
            0,
            format!("The module index does not import `{REGISTER}` from `{PACKAGE}`."),
            format!("Import `{REGISTER}` from `{PACKAGE}`."),
        );
    }
    if !has_module_map {
        report.at_offset(
            0,
            format!("The module index does not declare `{MODULE_MAP}` in `{PACKAGE}`."),
            format!("Declare module `{PACKAGE}` with interface `{MODULE_MAP}`."),
        );
    }
    report.finish()
}

fn check_index_import(report: &mut RuleReport<'_>, import: &ImportDeclaration<'_>) {
    if is_register_import(import) || is_module_import(import) {
        return;
    }
    let source = import.source.value.as_str().unwrap_or("");
    report.at_offset(
        import.span.start,
        format!("The module index import `{source}` is not `{REGISTER}` or a module class."),
        format!(
            "Import only `{REGISTER}` from `{PACKAGE}` and one module class from each module folder."
        ),
    );
}

fn is_register_import(import: &ImportDeclaration<'_>) -> bool {
    if import.source.value.as_str() != Some(PACKAGE) || import.import_kind.is_type() {
        return false;
    }
    single_import(import).is_some_and(|(imported, local, type_only)| {
        imported == REGISTER && local == REGISTER && !type_only
    })
}

fn is_module_import(import: &ImportDeclaration<'_>) -> bool {
    if import.import_kind.is_type() {
        return false;
    }
    let Some(source) = import.source.value.as_str() else {
        return false;
    };
    let Some(folder) = module_folder(source) else {
        return false;
    };
    let expected = format!("{}{BASE}", pascal_case(folder));
    single_import(import).is_some_and(|(imported, local, type_only)| {
        imported == expected && local == expected && !type_only
    })
}

fn single_import<'a>(import: &'a ImportDeclaration<'a>) -> Option<(&'a str, &'a str, bool)> {
    let specifiers = import.specifiers.as_ref()?;
    let [ImportDeclarationSpecifier::ImportSpecifier(item)] = specifiers.as_slice() else {
        return None;
    };
    Some((
        module_export_name(&item.imported),
        item.local.name.as_str(),
        item.import_kind.is_type(),
    ))
}

fn module_export_name<'a>(name: &'a ModuleExportName<'a>) -> &'a str {
    match name {
        ModuleExportName::IdentifierName(name) => name.name.as_str(),
        ModuleExportName::IdentifierReference(name) => name.name.as_str(),
        ModuleExportName::StringLiteral(name) => name.value.as_str().unwrap_or(""),
    }
}

fn module_folder(source: &str) -> Option<&str> {
    let name = source
        .strip_prefix("./")
        .or_else(|| source.strip_prefix("module/"))?;
    if name.is_empty() || name.contains('/') {
        return None;
    }
    Some(name)
}

fn is_register_call(statement: &oxc_ast::ast::ExpressionStatement<'_>) -> bool {
    let Expression::CallExpression(call) = &statement.expression else {
        return false;
    };
    matches!(&call.callee, Expression::Identifier(callee) if callee.name.as_str() == REGISTER)
}

fn is_module_map(module: &oxc_ast::ast::TSExternalModuleDeclaration<'_>) -> bool {
    if !module.declare || module.id.value.as_str() != Some(PACKAGE) {
        return false;
    }
    let Some(body) = &module.body else {
        return false;
    };
    matches!(
        body.body.as_slice(),
        [Statement::TSInterfaceDeclaration(interface)] if interface.id.name.as_str() == MODULE_MAP
    )
}

fn report_index_code(report: &mut RuleReport<'_>, offset: u32) {
    report.at_offset(
        offset,
        "`module/index.ts` has code outside the module registration shape.",
        format!(
            "Keep only module imports, the `{MODULE_MAP}` declaration, and the `{REGISTER}` calls."
        ),
    );
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

fn is_class(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ClassDeclaration(_) => true,
        Statement::ExportDeclaration(export) => {
            matches!(export.declaration, Declaration::ClassDeclaration(_))
        }
        _ => false,
    }
}

fn is_initial_store(statement: &Statement<'_>) -> bool {
    let variable = match statement {
        Statement::VariableDeclaration(variable) => variable,
        Statement::ExportDeclaration(export) => match &export.declaration {
            Declaration::VariableDeclaration(variable) => variable,
            _ => return false,
        },
        _ => return false,
    };
    is_initial_store_variable(variable)
}

fn is_initial_store_variable(variable: &VariableDeclaration<'_>) -> bool {
    if variable.kind != VariableDeclarationKind::Const || variable.declarations.len() != 1 {
        return false;
    }
    variable.declarations.iter().any(|declarator| {
        matches!(&declarator.id, BindingPattern::BindingIdentifier(id) if id.name.as_str() == INITIAL)
    })
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
