// `{name}` does not return `Result<T, Exception>`.
// Return `Result<T, Exception>`.
// Why a lint rule: A failure is an `Exception`. A trait impl keeps the trait's error type.

use std::fs;
use std::path::{Path, PathBuf};

use syn::spanned::Spanned;
use syn::{GenericArgument, ImplItem, Item, PathArguments, ReturnType, Type};

use crate::diagnostic::Diagnostic;
use crate::rule::finder::Finder;

const RULE: &str = "result-type";

pub fn check(path: &Path, _srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    check_source(path, &source)
}

pub(crate) fn check_source(path: &Path, source: &str) -> Vec<Diagnostic> {
    Finder::rust_source(path, source, RULE, |file, finder| {
        walk(&file.items, finder);
    })
}

fn walk(items: &[Item], finder: &mut Finder<'_>) {
    for item in items {
        match item {
            Item::Fn(item) => {
                report(
                    &item.sig.output,
                    &item.sig.ident.to_string(),
                    item.span(),
                    finder,
                );
            }
            Item::Impl(item) if item.trait_.is_none() => {
                for impl_item in &item.items {
                    let ImplItem::Fn(method) = impl_item else {
                        continue;
                    };
                    report(
                        &method.sig.output,
                        &method.sig.ident.to_string(),
                        method.span(),
                        finder,
                    );
                }
            }
            Item::Mod(item) => {
                if let Some((_, nested)) = &item.content {
                    walk(nested, finder);
                }
            }
            _ => {}
        }
    }
}

fn report(output: &ReturnType, name: &str, span: proc_macro2::Span, finder: &mut Finder<'_>) {
    let ReturnType::Type(_, ty) = output else {
        return;
    };
    if returns_exception(ty) {
        return;
    }
    if !is_result(ty) {
        return;
    }
    finder.at_span(
        span,
        format!("`{name}` does not return `Result<T, Exception>`."),
        "Return `Result<T, Exception>`.",
    );
}

fn returns_exception(ty: &Type) -> bool {
    let Some(error) = result_error(ty) else {
        return false;
    };
    is_exception(error)
}

fn is_result(ty: &Type) -> bool {
    result_path(bare(ty)).is_some()
}

fn result_error(ty: &Type) -> Option<&Type> {
    let args = result_path(bare(ty))?;
    let types: Vec<&Type> = args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect();
    match types.as_slice() {
        [_, error] => Some(*error),
        [_] => Some(bare(ty)),
        _ => None,
    }
}

fn result_path(ty: &Type) -> Option<&syn::punctuated::Punctuated<GenericArgument, syn::Token![,]>> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }
    match &segment.arguments {
        PathArguments::AngleBracketed(args) => Some(&args.args),
        PathArguments::None => None,
        PathArguments::Parenthesized(_) => None,
    }
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
