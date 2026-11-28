// `{specifier}` imports a module outside `index.tsx` and `module/index.ts`.
// Use the module through `this.modules` in a page context or a module. Only `index.tsx` in the src folder and `index.ts` in the module folder import a module.
// Why a lint rule: Only a page reaches a module, so a component cannot call a module directly.

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

const RULE: &str = "module-import";

const FOLDER: &str = "module";
const ENTRY: &str = "index.tsx";
const MODULE_ENTRY: &str = "module/index.ts";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let place = Place::new(file.path, file.srcs);
    if place.is_file(ENTRY) || place.is_file(MODULE_ENTRY) {
        return Vec::new();
    }
    let parts = place.parts();
    let Some((_, folders)) = parts.split_last() else {
        return Vec::new();
    };
    let own = match folders {
        [FOLDER, name, ..] => Some(*name),
        _ => None,
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut imports = Imports {
        report: &mut report,
        folders,
        own,
    };
    imports.visit_program(program);
    report.finish()
}

struct Imports<'a, 'b> {
    report: &'b mut RuleReport<'a>,
    folders: &'b [&'b str],
    own: Option<&'b str>,
}

impl<'ast> Visit<'ast> for Imports<'_, '_> {
    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'ast>) {
        if !it.import_kind.is_type() {
            self.specifier(&it.source);
        }
    }

    fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'ast>) {
        if !it.export_kind.is_type() {
            self.specifier(&it.source);
        }
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'ast>) {
        if !it.export_kind.is_type() {
            self.specifier(&it.source);
        }
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
        let target = if is_relative(text) {
            let Some(target) = resolve(self.folders, text) else {
                return;
            };
            target
        } else {
            text.split('/').collect()
        };
        let [FOLDER, rest @ ..] = &target[..] else {
            return;
        };
        if self.own.is_some() && rest.first().copied() == self.own {
            return;
        }
        self.report.at_offset(
            literal.span.start,
            format!("`{text}` imports a module outside `{ENTRY}` and `{MODULE_ENTRY}`."),
            format!(
                "Use the module through `this.modules` in a page context or a module. Only `{ENTRY}` in the src folder and `{MODULE_ENTRY}` import a module."
            ),
        );
    }
}

fn is_relative(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
}

fn resolve<'a>(folders: &[&'a str], specifier: &'a str) -> Option<Vec<&'a str>> {
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
    Some(target)
}
