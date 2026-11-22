use oxc_ast::ast::{
    BindingPattern, Class, Declaration, Expression, ImportDeclarationSpecifier, ModuleExportName,
    Program, Statement, TSType, TSTypeAnnotation, TSTypeName, VariableDeclaration,
    VariableDeclarationKind,
};

pub(crate) enum Constant<'a> {
    Absent,
    Untyped(u32),
    Typed(u32, &'a str),
}

pub(crate) fn imports<'a>(program: &'a Program<'_>, specifier: &str) -> Vec<(&'a str, &'a str)> {
    let mut names = Vec::new();
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        if import.source.value.as_str() != Some(specifier) {
            continue;
        }
        for item in import.specifiers.iter().flatten() {
            let ImportDeclarationSpecifier::ImportSpecifier(item) = item else {
                continue;
            };
            let imported = match &item.imported {
                ModuleExportName::IdentifierName(name) => name.name.as_str(),
                ModuleExportName::IdentifierReference(name) => name.name.as_str(),
                ModuleExportName::StringLiteral(name) => name.value.as_str().unwrap_or(""),
            };
            names.push((imported, item.local.name.as_str()));
        }
    }
    names
}

pub(crate) fn constant<'a>(program: &'a Program<'_>, name: &str) -> Constant<'a> {
    for statement in &program.body {
        let variable = match statement {
            Statement::VariableDeclaration(variable) => variable,
            Statement::ExportDeclaration(export) => match &export.declaration {
                Declaration::VariableDeclaration(variable) => variable,
                _ => continue,
            },
            _ => continue,
        };
        if let Some(found) = constant_in(variable, name) {
            return found;
        }
    }
    Constant::Absent
}

pub(crate) fn extends(class: &Class<'_>, base: &str) -> bool {
    match class
        .heritage
        .as_ref()
        .map(|heritage| heritage.expression.without_parentheses())
    {
        Some(Expression::Identifier(name)) => name.name.as_str() == base,
        _ => false,
    }
}

fn constant_in<'a>(variable: &'a VariableDeclaration<'_>, name: &str) -> Option<Constant<'a>> {
    if variable.kind != VariableDeclarationKind::Const {
        return None;
    }
    let declarator = variable.declarations.iter().find(|declarator| {
        matches!(&declarator.id, BindingPattern::BindingIdentifier(id) if id.name.as_str() == name)
    })?;
    let annotation = match &declarator.init {
        Some(Expression::ArrowFunctionExpression(arrow)) => arrow.return_type.as_ref(),
        _ => declarator.type_annotation.as_ref(),
    };
    let offset = declarator.span.start;
    Some(
        match annotation.and_then(|annotation| type_name(annotation)) {
            Some(name) => Constant::Typed(offset, name),
            None => Constant::Untyped(offset),
        },
    )
}

fn type_name<'a>(annotation: &'a TSTypeAnnotation<'_>) -> Option<&'a str> {
    let TSType::TSTypeReference(reference) = &annotation.type_annotation else {
        return None;
    };
    let TSTypeName::IdentifierReference(name) = &reference.type_name else {
        return None;
    };
    Some(name.name.as_str())
}
