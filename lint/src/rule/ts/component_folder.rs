// The file has no named export `{name}`.
// Export the component with the name of its folder.
// The props type `{name}` does not have the name `Props`.
// Name the props type `Props`.
// Why a lint rule: The folder name gives the component name, and each component has the same name for its props type.

use oxc_ast::ast::{
    BindingPattern, Declaration, Expression, FormalParameters, Statement, TSType, TSTypeName,
};

use crate::file::place::Place;
use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "component-folder";

const PROPS: &str = "Props";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let place = Place::new(file.path, file.srcs);
    let ["component", .., folder, "index.tsx"] = place.parts()[..] else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let Some(params) = program
        .body
        .iter()
        .find_map(|statement| component(statement, folder))
    else {
        report.at_offset(
            0,
            format!("The file has no named export `{folder}`."),
            "Export the component with the name of its folder.",
        );
        return report.finish();
    };
    if let Some(params) = params
        && let Some(first) = params.items.first()
        && let Some(annotation) = &first.type_annotation
        && let TSType::TSTypeReference(reference) = &annotation.type_annotation
        && let TSTypeName::IdentifierReference(name) = &reference.type_name
        && name.name.as_str() != PROPS
    {
        report.at_offset(
            name.span.start,
            format!(
                "The props type `{}` does not have the name `{PROPS}`.",
                name.name
            ),
            format!("Name the props type `{PROPS}`."),
        );
    }
    report.finish()
}

fn component<'a, 'ast>(
    statement: &'a Statement<'ast>,
    name: &str,
) -> Option<Option<&'a FormalParameters<'ast>>> {
    let Statement::ExportDeclaration(export) = statement else {
        return None;
    };
    match &export.declaration {
        Declaration::FunctionDeclaration(function) => function
            .id
            .as_ref()
            .is_some_and(|id| id.name.as_str() == name)
            .then_some(Some(&*function.params)),
        Declaration::VariableDeclaration(variable) => variable
            .declarations
            .iter()
            .find(|declarator| {
                matches!(&declarator.id, BindingPattern::BindingIdentifier(id) if id.name.as_str() == name)
            })
            .map(|declarator| match &declarator.init {
                Some(Expression::ArrowFunctionExpression(arrow)) => Some(&*arrow.params),
                Some(Expression::FunctionExpression(function)) => Some(&*function.params),
                _ => None,
            }),
        _ => None,
    }
}
