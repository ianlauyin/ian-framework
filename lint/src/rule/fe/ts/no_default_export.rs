// The file has a default export.
// Use a named export.
// Why a lint rule: A named export keeps one name for the item.

use std::fs;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::{ModuleExportName, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "no-default-export";

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    check_source(path, &source)
}

pub(crate) fn check_source(path: &Path, source: &str) -> Vec<Diagnostic> {
    let Ok(source_type) = SourceType::from_path(path) else {
        return Vec::new();
    };
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    let mut finder = Finder::new(path, source);
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                report(&mut finder, declaration.span.start);
            }
            Statement::ExportNamedDeclaration(declaration) => {
                report_specifiers(&mut finder, &declaration.specifiers);
            }
            Statement::ExportFromDeclaration(declaration) => {
                report_specifiers(&mut finder, &declaration.specifiers);
            }
            _ => {}
        }
    }
    finder.finish()
}

fn report_specifiers(finder: &mut Finder<'_>, specifiers: &[oxc_ast::ast::ExportSpecifier<'_>]) {
    for specifier in specifiers {
        if is_default_name(&specifier.exported) {
            report(finder, specifier.span.start);
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

fn report(finder: &mut Finder<'_>, offset: u32) {
    finder.report(
        offset,
        RULE,
        "The file has a default export.",
        "Use a named export.",
    );
}
