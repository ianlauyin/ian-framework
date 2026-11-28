// The module index import `{specifier}` is not `registerModule` or a module class.
// Import only `registerModule` from `ian-framework-fe` and one module class from each module folder.
// The module index does not import `registerModule` from `ian-framework-fe`.
// Import `registerModule` from `ian-framework-fe`.
// The module index does not declare `ModuleMap` in `ian-framework-fe`.
// Declare module `ian-framework-fe` with interface `ModuleMap`.
// The module index does not register `{Module}Module`.
// Call `registerModule("{module}", new {Module}Module());`.
// `module/index.ts` has code outside the module registration shape.
// Keep only module imports, the `ModuleMap` declaration, and the `registerModule` calls.
// Why a lint rule: Module registration has one shape, so each module is in `ModuleMap` and has one instance.

use oxc_ast::ast::{
    Argument, Expression, ImportDeclaration, ImportDeclarationSpecifier, ModuleExportName,
    Statement,
};
use oxc_span::GetSpan;

use crate::file::place::Place;
use crate::file::ts::declaration::imports;
use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "module-registration";

const BASE: &str = "Module";
const REGISTER: &str = "registerModule";
const MODULE_MAP: &str = "ModuleMap";
const PACKAGE: &str = "ian-framework-fe";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let place = Place::new(file.path, file.srcs);
    if !place.is_file("module/index.ts") {
        return Vec::new();
    }
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut has_module_map = false;
    let mut modules = Vec::new();
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                if let Some(module) = check_index_import(&mut report, import) {
                    modules.push(module);
                }
            }
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
    for module in modules {
        if !program
            .body
            .iter()
            .any(|statement| is_module_registration(statement, &module.class, &module.key))
        {
            report.at_offset(
                module.offset,
                format!("The module index does not register `{}`.", module.class),
                format!(
                    "Call `{REGISTER}(\"{}\", new {}());`.",
                    module.key, module.class
                ),
            );
        }
    }
    report.finish()
}

struct ModuleImport {
    class: String,
    key: String,
    offset: u32,
}

fn check_index_import(
    report: &mut RuleReport<'_>,
    import: &ImportDeclaration<'_>,
) -> Option<ModuleImport> {
    if is_register_import(import) {
        return None;
    }
    if let Some(module) = module_import(import) {
        return Some(module);
    }
    let source = import.source.value.as_str().unwrap_or("");
    report.at_offset(
        import.span.start,
        format!("The module index import `{source}` is not `{REGISTER}` or a module class."),
        format!(
            "Import only `{REGISTER}` from `{PACKAGE}` and one module class from each module folder."
        ),
    );
    None
}

fn is_register_import(import: &ImportDeclaration<'_>) -> bool {
    if import.source.value.as_str() != Some(PACKAGE) || import.import_kind.is_type() {
        return false;
    }
    single_import(import).is_some_and(|(imported, local, type_only)| {
        imported == REGISTER && local == REGISTER && !type_only
    })
}

fn module_import(import: &ImportDeclaration<'_>) -> Option<ModuleImport> {
    if import.import_kind.is_type() {
        return None;
    }
    let source = import.source.value.as_str()?;
    let folder = module_folder(source)?;
    let class = format!("{}{BASE}", pascal_case(folder));
    let valid = single_import(import).is_some_and(|(imported, local, type_only)| {
        imported == class && local == class && !type_only
    });
    if !valid {
        return None;
    }
    Some(ModuleImport {
        class,
        key: camel_case(folder),
        offset: import.span.start,
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

fn is_module_registration(statement: &Statement<'_>, class: &str, key: &str) -> bool {
    let Statement::ExpressionStatement(statement) = statement else {
        return false;
    };
    let Expression::CallExpression(call) = &statement.expression else {
        return false;
    };
    let Expression::Identifier(callee) = &call.callee else {
        return false;
    };
    let [Argument::StringLiteral(name), Argument::NewExpression(new)] = call.arguments.as_slice()
    else {
        return false;
    };
    let Expression::Identifier(module) = &new.callee else {
        return false;
    };
    callee.name.as_str() == REGISTER
        && name.value.as_str() == Some(key)
        && module.name.as_str() == class
        && new.arguments.is_empty()
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
