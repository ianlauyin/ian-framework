// `{specifier}` goes through `node_modules`.
// Import the package by its name.
// Why a lint rule: A `node_modules` path is an install path. The package name is the import path.

use oxc_ast::ast::{
    ExportAllDeclaration, ExportFromDeclaration, Expression, ImportDeclaration, ImportExpression,
    StringLiteral, TSExternalModuleReference,
};
use oxc_ast_visit::Visit;
use oxc_ast_visit::walk::walk_import_expression;

use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::ts::file::TsFile;
use crate::rule::report::Report;

const RULE: &str = "no-restricted-imports";

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = Report::new(file.path, file.source, RULE);
    let mut imports = Imports {
        report: &mut report,
    };
    imports.visit_program(program);
    report.finish()
}

struct Imports<'a, 'b> {
    report: &'b mut Report<'a>,
}

impl<'ast> Visit<'ast> for Imports<'_, '_> {
    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'ast>) {
        self.specifier(&it.source);
    }

    fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'ast>) {
        self.specifier(&it.source);
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'ast>) {
        self.specifier(&it.source);
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'ast>) {
        if let Expression::StringLiteral(literal) = &it.source {
            self.specifier(literal);
        }
        walk_import_expression(self, it);
    }

    fn visit_ts_external_module_reference(&mut self, it: &TSExternalModuleReference<'ast>) {
        self.specifier(&it.expression);
    }
}

impl Imports<'_, '_> {
    fn specifier(&mut self, literal: &StringLiteral<'_>) {
        let Some(text) = literal.value.as_str() else {
            return;
        };
        if !one_segment_after_node_modules(text) {
            return;
        }
        self.report.at_offset(
            literal.span.start,
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
