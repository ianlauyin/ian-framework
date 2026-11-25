// A class or an interface in an ajax file uses `extends`.
// Write the members out in full.
// Why a lint rule: An ajax file shows the whole request and response in one place, and `extends` hides part of it.

use oxc_ast::ast::{Class, TSInterfaceDeclaration};
use oxc_ast_visit::Visit;
use oxc_ast_visit::walk::{walk_class, walk_ts_interface_declaration};

use crate::file::place::Place;
use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "ajax-no-extends";

const FIX: &str = "Write the members out in full.";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let place = Place::new(file.path, file.srcs);
    if !matches!(&place.parts()[..], ["util", "ajax", _, ..]) {
        return Vec::new();
    }
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut declarations = Declarations {
        report: &mut report,
    };
    declarations.visit_program(program);
    report.finish()
}

struct Declarations<'a, 'b> {
    report: &'b mut RuleReport<'a>,
}

impl<'ast> Visit<'ast> for Declarations<'_, '_> {
    fn visit_class(&mut self, it: &Class<'ast>) {
        if it.heritage.is_some() {
            self.report
                .at_offset(it.span.start, "A class uses `extends`.".to_string(), FIX);
        }
        walk_class(self, it);
    }

    fn visit_ts_interface_declaration(&mut self, it: &TSInterfaceDeclaration<'ast>) {
        if let Some(first) = it.extends.first() {
            self.report.at_offset(
                first.span.start,
                format!("The interface `{}` uses `extends`.", it.id.name),
                FIX,
            );
        }
        walk_ts_interface_declaration(self, it);
    }
}
