// The override method `{name}` is before the constructor, or after a method that is not an override.
// Move the override methods below the constructor and above the other methods of the class.
// Why a lint rule: That order keeps the constructor at the top of a class, and the methods that change the base class directly below it.

use oxc_ast::ast::{Class, ClassElement, MethodDefinition, MethodDefinitionKind};
use oxc_ast_visit::Visit;
use oxc_ast_visit::walk::walk_class;

use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "override-method-order";

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
        let constructor = methods
            .iter()
            .position(|method| method.kind == MethodDefinitionKind::Constructor);
        let mut other = false;
        for (index, method) in methods.iter().enumerate() {
            if method.kind == MethodDefinitionKind::Constructor {
                continue;
            }
            if !method.r#override {
                other = true;
                continue;
            }
            let place = if constructor.is_some_and(|constructor| index < constructor) {
                "before the constructor"
            } else if other {
                "after a method that is not an override"
            } else {
                continue;
            };
            let name = match (method.key.private_name(), method.key.static_name()) {
                (Some(name), _) => format!("`#{name}`"),
                (None, Some(name)) => format!("`{}`", name.display()),
                (None, None) => "a method".to_string(),
            };
            self.report.at_offset(
                method.span.start,
                format!("The override method {name} is {place}."),
                "Move the override methods below the constructor and above the other methods of the class.",
            );
        }
    }
}
