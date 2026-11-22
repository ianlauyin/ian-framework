// The action name of `Logger.capture` is not `{name}`.
// Write the action name as the page name, `/`, and the name of the method.
// Why a lint rule: The action name in a log event gives the page and the method that made the event.

use oxc_ast::ast::{Argument, CallExpression, Class, Expression, MethodDefinition};
use oxc_ast_visit::Visit;
use oxc_ast_visit::walk::{walk_call_expression, walk_class, walk_method_definition};
use oxc_span::GetSpan;

use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "capture-action-name";

const BASE: &str = "PageContext";
const SUFFIX: &str = "Page";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut calls = Calls {
        report: &mut report,
        page: None,
        method: None,
    };
    calls.visit_program(program);
    report.finish()
}

struct Calls<'a, 'b> {
    report: &'b mut RuleReport<'a>,
    page: Option<String>,
    method: Option<String>,
}

impl<'ast> Visit<'ast> for Calls<'_, '_> {
    fn visit_class(&mut self, it: &Class<'ast>) {
        let page = self.page.take();
        let method = self.method.take();
        self.page = page_name(it);
        walk_class(self, it);
        self.page = page;
        self.method = method;
    }

    fn visit_method_definition(&mut self, it: &MethodDefinition<'ast>) {
        let method = self.method.take();
        self.method = it.key.static_name().map(|name| name.display().to_string());
        walk_method_definition(self, it);
        self.method = method;
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'ast>) {
        if let (Some(page), Some(method)) = (&self.page, &self.method)
            && is_capture(&it.callee)
        {
            let expected = format!("{page}/{method}");
            let found = match it.arguments.first() {
                Some(Argument::StringLiteral(literal)) => literal.value.as_str(),
                _ => None,
            };
            if found != Some(expected.as_str()) {
                self.report.at_offset(
                    it.arguments
                        .first()
                        .map_or(it.span.start, |argument| argument.span().start),
                    format!("The action name of `Logger.capture` is not `{expected}`."),
                    "Write the action name as the page name, `/`, and the name of the method.",
                );
            }
        }
        walk_call_expression(self, it);
    }
}

fn page_name(class: &Class<'_>) -> Option<String> {
    let extends = match class
        .heritage
        .as_ref()
        .map(|heritage| heritage.expression.without_parentheses())
    {
        Some(Expression::Identifier(base)) => base.name.as_str() == BASE,
        _ => false,
    };
    if !extends {
        return None;
    }
    let name = class.id.as_ref()?.name.as_str();
    let name = name.strip_suffix(SUFFIX).unwrap_or(name);
    let mut chars = name.chars();
    let first = chars.next()?;
    Some(first.to_lowercase().chain(chars).collect())
}

fn is_capture(callee: &Expression<'_>) -> bool {
    let Expression::StaticMemberExpression(member) = callee.without_parentheses() else {
        return false;
    };
    member.property.name.as_str() == "capture"
        && matches!(&member.object, Expression::Identifier(object) if object.name.as_str() == "Logger")
}
