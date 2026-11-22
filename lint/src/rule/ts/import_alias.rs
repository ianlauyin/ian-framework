// `{specifier}` is a relative path into `{folder}/`.
// Import the file with the alias `{folder}`.
// `{specifier}` is an alias path in `{folder}/`.
// Use a relative path.
// Why a lint rule: An alias path stays valid when the importing file moves. A file in `component/` moves with its folder.

use oxc_ast::ast::{
    ExportAllDeclaration, ExportFromDeclaration, Expression, ImportDeclaration, ImportExpression,
    StringLiteral,
};
use oxc_ast_visit::Visit;
use oxc_ast_visit::walk::walk_import_expression;

use crate::file::place::Place;
use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "import-alias";

const ALIASES: [&str; 4] = ["component", "util", "module", "i18n"];
const RELATIVE_INSIDE: [&str; 1] = ["component"];

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let place = Place::new(file.path, file.srcs);
    let parts = place.parts();
    let Some((_, folders)) = parts.split_last() else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut imports = Imports {
        report: &mut report,
        folders,
    };
    imports.visit_program(program);
    report.finish()
}

struct Imports<'a, 'b> {
    report: &'b mut RuleReport<'a>,
    folders: &'b [&'b str],
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
}

impl Imports<'_, '_> {
    fn specifier(&mut self, literal: &StringLiteral<'_>) {
        let Some(text) = literal.value.as_str() else {
            return;
        };
        let own = self.folders.first().copied();
        if is_relative(text) {
            let Some(folder) = target_folder(self.folders, text) else {
                return;
            };
            if ALIASES.contains(&folder) && own != Some(folder) {
                self.report.at_offset(
                    literal.span.start,
                    format!("`{text}` is a relative path into `{folder}/`."),
                    format!("Import the file with the alias `{folder}`."),
                );
            }
            return;
        }
        let folder = text.split('/').next().unwrap_or(text);
        if RELATIVE_INSIDE.contains(&folder) && own == Some(folder) {
            self.report.at_offset(
                literal.span.start,
                format!("`{text}` is an alias path in `{folder}/`."),
                "Use a relative path.",
            );
        }
    }
}

fn is_relative(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
}

fn target_folder<'a>(folders: &[&'a str], specifier: &'a str) -> Option<&'a str> {
    let mut target = folders.to_vec();
    for part in specifier.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                target.pop()?;
            }
            _ => target.push(part),
        }
    }
    target.first().copied()
}
