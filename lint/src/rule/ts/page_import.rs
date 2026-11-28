// `{specifier}` imports a sibling page.
// Move the shared code to a parent page folder, to `component/`, or to `util/`.
// Why a lint rule: A page stays apart from its sibling pages, so a change in one page does not break a different page.

use std::path::Path;

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

const RULE: &str = "page-import";

const FOLDER: &str = "page";

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
    let [FOLDER, rest @ ..] = folders else {
        return Vec::new();
    };
    let page = pages(rest);
    if page.is_empty() {
        return Vec::new();
    }
    let Some(root) = file
        .path
        .parent()
        .and_then(|dir| dir.ancestors().nth(rest.len()))
    else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut imports = Imports {
        report: &mut report,
        folders,
        page,
        root,
    };
    imports.visit_program(program);
    report.finish()
}

struct Imports<'a, 'b> {
    report: &'b mut RuleReport<'a>,
    folders: &'b [&'b str],
    page: &'b [&'b str],
    root: &'b Path,
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
        let target = if is_relative(text) {
            let Some(target) = resolve(self.folders, text) else {
                return;
            };
            target
        } else {
            text.split('/').collect()
        };
        let [FOLDER, target @ ..] = &target[..] else {
            return;
        };
        let Some(at) = self
            .page
            .iter()
            .zip(pages(target))
            .position(|(own, other)| own != other)
        else {
            return;
        };
        if at + 1 == target.len() && !self.root.join(target.join("/")).is_dir() {
            return;
        }
        self.report.at_offset(
            literal.span.start,
            format!("`{text}` imports a sibling page."),
            "Move the shared code to a parent page folder, to `component/`, or to `util/`."
                .to_string(),
        );
    }
}

fn pages<'a>(parts: &'a [&'a str]) -> &'a [&'a str] {
    let count = parts
        .iter()
        .take_while(|name| name.starts_with(|first: char| first.is_ascii_lowercase()))
        .count();
    &parts[..count]
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
