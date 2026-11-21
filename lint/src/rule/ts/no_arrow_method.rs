// A method of a class that extends `PageContext` or `Module` is an arrow function.
// Write the method as a class method.
// Why a lint rule: An arrow function property is an instance field. It is not on the prototype, so a subclass cannot override it and call `super`.

use oxc_ast::ast::{Class, ClassElement, Expression};
use oxc_ast_visit::Visit;
use oxc_ast_visit::walk::walk_class;

use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "no-arrow-method";

const BASES: [&str; 2] = ["PageContext", "Module"];

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut classes = Classes {
        report: &mut report,
    };
    classes.visit_program(program);
    report.finish()
}

struct Classes<'a, 'b> {
    report: &'b mut RuleReport<'a>,
}

impl<'ast> Visit<'ast> for Classes<'_, '_> {
    fn visit_class(&mut self, it: &Class<'ast>) {
        if extends_base(it) {
            self.members(it);
        }
        walk_class(self, it);
    }
}

impl Classes<'_, '_> {
    fn members(&mut self, class: &Class<'_>) {
        for element in &class.body.body {
            let ClassElement::PropertyDefinition(property) = element else {
                continue;
            };
            let Some(value) = &property.value else {
                continue;
            };
            if !matches!(
                value.without_parentheses(),
                Expression::ArrowFunctionExpression(_)
            ) {
                continue;
            }
            let name = property.key.static_name().map_or_else(
                || "A method".to_string(),
                |name| format!("`{}`", name.display()),
            );
            self.report.at_offset(
                property.span.start,
                format!("{name} is an arrow function."),
                "Write the method as a class method.",
            );
        }
    }
}

fn extends_base(class: &Class<'_>) -> bool {
    match class
        .heritage
        .as_ref()
        .map(|heritage| heritage.expression.without_parentheses())
    {
        Some(Expression::Identifier(name)) => BASES.contains(&name.name.as_str()),
        _ => false,
    }
}
