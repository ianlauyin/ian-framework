// The file has no named export `{name}`.
// Export the component with the name of its folder.
// The props type `{name}` does not have the name `Props`.
// Name the props type `Props`.
// The props type does not have the name `Props`.
// Name the props type `Props`, and declare it as an interface.
// The props type `Props` is not an interface.
// Declare the props type as an interface.
// The props type `Props` is not exported.
// Export the props type.
// The props type `Props` is after the component.
// Declare the props type before the component.
// Why a lint rule: The folder name gives the component name, and each component has the same name for its props type. An exported props type before the component lets a caller read the props first and import them.

use oxc_ast::ast::{
    BindingPattern, Declaration, Expression, FormalParameters, Statement, TSType, TSTypeName,
};
use oxc_span::GetSpan;

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
    let Some((start, params)) = program.body.iter().find_map(|statement| {
        component(statement, folder).map(|params| (statement.span().start, params))
    }) else {
        report.at_offset(
            0,
            format!("The file has no named export `{folder}`."),
            "Export the component with the name of its folder.",
        );
        return report.finish();
    };
    let Some(annotation) = params
        .and_then(|params| params.items.first())
        .and_then(|first| first.type_annotation.as_ref())
    else {
        return report.finish();
    };
    let props_type = &annotation.type_annotation;
    let TSType::TSTypeReference(reference) = props_type else {
        report.at_offset(
            props_type.span().start,
            format!("The props type does not have the name `{PROPS}`."),
            format!("Name the props type `{PROPS}`, and declare it as an interface."),
        );
        return report.finish();
    };
    let TSTypeName::IdentifierReference(name) = &reference.type_name else {
        report.at_offset(
            reference.span.start,
            format!("The props type does not have the name `{PROPS}`."),
            format!("Name the props type `{PROPS}`, and declare it as an interface."),
        );
        return report.finish();
    };
    if name.name.as_str() != PROPS {
        report.at_offset(
            name.span.start,
            format!(
                "The props type `{}` does not have the name `{PROPS}`.",
                name.name
            ),
            format!("Name the props type `{PROPS}`."),
        );
        return report.finish();
    }
    check_props(&mut report, &program.body, start);
    report.finish()
}

fn check_props(report: &mut RuleReport<'_>, body: &[Statement<'_>], component_start: u32) {
    let Some(props) = body.iter().find_map(props_declaration) else {
        return;
    };
    let offset = props.offset;
    if !props.interface {
        report.at_offset(
            offset,
            format!("The props type `{PROPS}` is not an interface."),
            "Declare the props type as an interface.",
        );
    }
    if !props.exported {
        report.at_offset(
            offset,
            format!("The props type `{PROPS}` is not exported."),
            "Export the props type.",
        );
    }
    if offset > component_start {
        report.at_offset(
            offset,
            format!("The props type `{PROPS}` is after the component."),
            "Declare the props type before the component.",
        );
    }
}

struct PropsDeclaration {
    offset: u32,
    exported: bool,
    interface: bool,
}

fn props_declaration(statement: &Statement<'_>) -> Option<PropsDeclaration> {
    let (declaration, exported) = match statement {
        Statement::ExportDeclaration(export) => (&export.declaration, true),
        _ => (statement.as_declaration()?, false),
    };
    let (name, interface) = match declaration {
        Declaration::TSInterfaceDeclaration(interface) => (interface.id.name.as_str(), true),
        Declaration::TSTypeAliasDeclaration(alias) => (alias.id.name.as_str(), false),
        _ => return None,
    };
    (name == PROPS).then_some(PropsDeclaration {
        offset: statement.span().start,
        exported,
        interface,
    })
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
