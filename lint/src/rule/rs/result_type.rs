// `{name}` does not return `Result<T, Exception>`.
// Return `Result<T, Exception>`.
// Why a lint rule: A failure is an `Exception`. A trait impl keeps the trait's error type.

use syn::spanned::Spanned;
use syn::{GenericArgument, ImplItem, Item, PathArguments, PathSegment, ReturnType, Type};

use crate::file::place::Place;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "result-type";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    if Place::new(file.path, file.srcs).in_folder("tests") {
        return Vec::new();
    }
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    walk(&ast.items, &mut report);
    report.finish()
}

fn walk(items: &[Item], report: &mut RuleReport<'_>) {
    for item in items {
        match item {
            Item::Fn(item) => {
                report_return(
                    &item.sig.output,
                    &item.sig.ident.to_string(),
                    item.span(),
                    report,
                );
            }
            Item::Impl(item) if item.trait_.is_none() => {
                for impl_item in &item.items {
                    let ImplItem::Fn(method) = impl_item else {
                        continue;
                    };
                    report_return(
                        &method.sig.output,
                        &method.sig.ident.to_string(),
                        method.span(),
                        report,
                    );
                }
            }
            Item::Mod(item) => {
                if let Some((_, nested)) = &item.content {
                    walk(nested, report);
                }
            }
            _ => {}
        }
    }
}

fn report_return(
    output: &ReturnType,
    name: &str,
    span: proc_macro2::Span,
    report: &mut RuleReport<'_>,
) {
    let ReturnType::Type(_, ty) = output else {
        return;
    };
    let Some(result) = result_segment(ty) else {
        return;
    };
    if returns_exception(result) {
        return;
    }
    report.at_span(
        span,
        format!("`{name}` does not return `Result<T, Exception>`."),
        "Return `Result<T, Exception>`.",
    );
}

fn result_segment(ty: &Type) -> Option<&PathSegment> {
    let Type::Path(path) = bare(ty) else {
        return None;
    };
    path.path
        .segments
        .last()
        .filter(|segment| segment.ident == "Result")
}

fn returns_exception(result: &PathSegment) -> bool {
    let PathArguments::AngleBracketed(args) = &result.arguments else {
        return false;
    };
    let types: Vec<&Type> = args
        .args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect();
    matches!(types.as_slice(), [_, error] if is_exception(error))
}

fn is_exception(ty: &Type) -> bool {
    let Type::Path(path) = bare(ty) else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Exception" && segment.arguments.is_empty())
}

fn bare(ty: &Type) -> &Type {
    match ty {
        Type::Group(ty) => bare(&ty.elem),
        Type::Paren(ty) => bare(&ty.elem),
        _ => ty,
    }
}
