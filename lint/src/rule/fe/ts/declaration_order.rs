// This item is out of declaration order.
// Put imports first, then private types and constants, then exported items, then private interfaces, then private functions.
// Why a lint rule: That order keeps the exported items in one place.

use std::fs;
use std::path::Path;

use oxc_ast::ast::Statement;
use oxc_span::GetSpan;

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "declaration-order";
const IMPORT: u8 = 0;
const PRIVATE_TYPE_OR_CONSTANT: u8 = 1;
const EXPORT: u8 = 2;
const PRIVATE_INTERFACE: u8 = 3;
const PRIVATE_FUNCTION: u8 = 4;

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    check_source(path, &source)
}

pub(crate) fn check_source(path: &Path, source: &str) -> Vec<Diagnostic> {
    Finder::typescript_source(path, source, RULE, |program, finder| {
        let mut last: Option<u8> = None;
        for statement in &program.body {
            let Some(group) = group(statement) else {
                report(finder, statement.span().start);
                continue;
            };
            if last.is_some_and(|previous| group < previous) {
                report(finder, statement.span().start);
            }
            last = Some(last.map_or(group, |previous| previous.max(group)));
        }
    })
}

fn group(statement: &Statement<'_>) -> Option<u8> {
    match statement {
        Statement::ImportDeclaration(_) | Statement::TSImportEqualsDeclaration(_) => Some(IMPORT),
        Statement::VariableDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => {
            Some(PRIVATE_TYPE_OR_CONSTANT)
        }
        Statement::ExportAllDeclaration(_)
        | Statement::ExportDefaultDeclaration(_)
        | Statement::ExportNamedDeclaration(_)
        | Statement::ExportDeclaration(_)
        | Statement::ExportFromDeclaration(_)
        | Statement::TSExportAssignment(_)
        | Statement::TSNamespaceExportDeclaration(_) => Some(EXPORT),
        Statement::TSInterfaceDeclaration(_) => Some(PRIVATE_INTERFACE),
        Statement::FunctionDeclaration(_) => Some(PRIVATE_FUNCTION),
        _ => None,
    }
}

fn report(finder: &mut Finder<'_>, offset: u32) {
    finder.report(
        offset,
        "This item is out of declaration order.",
        "Put imports first, then private types and constants, then exported items, then private interfaces, then private functions.",
    );
}
