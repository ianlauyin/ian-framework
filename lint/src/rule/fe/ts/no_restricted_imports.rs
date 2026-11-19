// `{specifier}` goes through `node_modules`.
// Import the package by its name.
// Why a lint rule: A `node_modules` path is an install path. The package name is the import path.

use std::fs;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ExportAllDeclaration, ExportFromDeclaration, Expression, ImportDeclaration, ImportExpression,
    StringLiteral, TSExternalModuleReference,
};
use oxc_ast_visit::Visit;
use oxc_ast_visit::walk::walk_import_expression;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "no-restricted-imports";

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
    let mut imports = Imports {
        finder: Finder::new(path, source),
    };
    imports.visit_program(&parsed.program);
    imports.finder.finish()
}

struct Imports<'a> {
    finder: Finder<'a>,
}

impl<'a> Visit<'a> for Imports<'a> {
    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        self.specifier(&it.source);
    }

    fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'a>) {
        self.specifier(&it.source);
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
        self.specifier(&it.source);
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        if let Expression::StringLiteral(literal) = &it.source {
            self.specifier(literal);
        }
        walk_import_expression(self, it);
    }

    fn visit_ts_external_module_reference(&mut self, it: &TSExternalModuleReference<'a>) {
        self.specifier(&it.expression);
    }
}

impl Imports<'_> {
    fn specifier(&mut self, literal: &StringLiteral<'_>) {
        let Some(text) = literal.value.as_str() else {
            return;
        };
        if !one_segment_after_node_modules(text) {
            return;
        }
        self.finder.report(
            literal.span.start,
            RULE,
            format!("`{text}` goes through `node_modules`."),
            "Import the package by its name.",
        );
    }
}

fn one_segment_after_node_modules(specifier: &str) -> bool {
    let parts: Vec<&str> = specifier.split('/').collect();
    parts.iter().enumerate().any(|(index, part)| {
        *part == "node_modules" && parts.len() == index + 2 && !parts[index + 1].is_empty()
    })
}
