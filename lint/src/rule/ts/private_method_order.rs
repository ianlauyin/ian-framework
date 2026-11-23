// The private method `{name}` is before a method that is not private.
// Move the private methods below all the other methods of the class.
// Why a lint rule: That order keeps the public methods of a class in one place.

use oxc_ast::ast::{Class, ClassElement, MethodDefinition, TSAccessibility};
use oxc_ast_visit::Visit;
use oxc_ast_visit::walk::walk_class;

use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "private-method-order";

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
        self.members(it);
        walk_class(self, it);
    }
}

impl Classes<'_, '_> {
    fn members(&mut self, class: &Class<'_>) {
        let methods: Vec<&MethodDefinition> = class
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                ClassElement::MethodDefinition(method) => Some(&**method),
                _ => None,
            })
            .collect();
        let Some(last) = methods.iter().rposition(|method| !is_private(method)) else {
            return;
        };
        for method in methods[..last].iter().filter(|method| is_private(method)) {
            let name = match (method.key.private_name(), method.key.static_name()) {
                (Some(name), _) => format!("`#{name}`"),
                (None, Some(name)) => format!("`{}`", name.display()),
                (None, None) => "a method".to_string(),
            };
            self.report.at_offset(
                method.span.start,
                format!("The private method {name} is before a method that is not private."),
                "Move the private methods below all the other methods of the class.",
            );
        }
    }
}

fn is_private(method: &MethodDefinition<'_>) -> bool {
    method.key.is_private_identifier() || method.accessibility == Some(TSAccessibility::Private)
}
