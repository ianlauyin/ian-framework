// The type `{name}` in the `Json` {request} of the handler is not camelCase.
// Add `#[serde(rename_all = "camelCase")]` to the type.
// Why a lint rule: Each JSON request and response uses the same key case.

use syn::{
    Attribute, FnArg, GenericArgument, Item, ItemFn, PathArguments, ReturnType, Type, TypePath,
};

use crate::file::place::Place;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "json-camel-case";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let place = Place::new(file.path, file.srcs);
    let parts = place.parts();
    let Some((_, folders)) = parts.split_last() else {
        return Vec::new();
    };
    if place.in_folder("tests") || !folders.contains(&"router") {
        return Vec::new();
    }
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    for item in &ast.items {
        let Item::Fn(handler) = item else {
            continue;
        };
        if handler.sig.asyncness.is_some() {
            check_handler(handler, &ast.items, &mut report);
        }
    }
    report.finish()
}

fn check_handler(handler: &ItemFn, items: &[Item], report: &mut RuleReport<'_>) {
    for arg in &handler.sig.inputs {
        if let FnArg::Typed(arg) = arg {
            check_type(&arg.ty, false, items, report);
        }
    }
    if let ReturnType::Type(_, ty) = &handler.sig.output {
        check_type(ty, true, items, report);
    }
}

fn check_type(ty: &Type, response: bool, items: &[Item], report: &mut RuleReport<'_>) {
    let Type::Path(TypePath { path, .. }) = ty else {
        return;
    };
    let Some(last) = path.segments.last() else {
        return;
    };
    let json = last.ident == "Json";
    for arg in generic_types(&last.arguments) {
        if json {
            check_named(arg, response, items, report);
        } else {
            check_type(arg, response, items, report);
        }
    }
}

fn check_named(ty: &Type, response: bool, items: &[Item], report: &mut RuleReport<'_>) {
    let Type::Path(TypePath { path, .. }) = ty else {
        return;
    };
    let Some(last) = path.segments.last() else {
        return;
    };
    for arg in generic_types(&last.arguments) {
        check_named(arg, response, items, report);
    }
    let Some((attrs, ident)) = items.iter().find_map(|item| local_type(item, &last.ident)) else {
        return;
    };
    if !camel_case(attrs, response) {
        let (name, request) = (ident, if response { "response" } else { "request" });
        report.at_span(
            ident.span(),
            format!("The type `{name}` in the `Json` {request} of the handler is not camelCase."),
            "Add `#[serde(rename_all = \"camelCase\")]` to the type.",
        );
    }
}

fn generic_types(arguments: &PathArguments) -> impl Iterator<Item = &Type> {
    let args = match arguments {
        PathArguments::AngleBracketed(args) => Some(&args.args),
        _ => None,
    };
    args.into_iter().flatten().filter_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn local_type<'a>(item: &'a Item, name: &syn::Ident) -> Option<(&'a [Attribute], &'a syn::Ident)> {
    match item {
        Item::Struct(item) if item.ident == *name => Some((&item.attrs, &item.ident)),
        Item::Enum(item) if item.ident == *name => Some((&item.attrs, &item.ident)),
        _ => None,
    }
}

fn camel_case(attrs: &[Attribute], response: bool) -> bool {
    let direction = if response { "serialize" } else { "deserialize" };
    let mut found = false;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        let _ = attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("rename_all") {
                if meta.input.peek(syn::Token![=]) {
                    meta.value()?.parse::<syn::Expr>()?;
                } else if meta.input.peek(syn::token::Paren) {
                    let content;
                    syn::parenthesized!(content in meta.input);
                    content.parse::<proc_macro2::TokenStream>()?;
                }
                return Ok(());
            }
            if meta.input.peek(syn::Token![=]) {
                found |= meta.value()?.parse::<syn::LitStr>()?.value() == "camelCase";
                return Ok(());
            }
            meta.parse_nested_meta(|inner| {
                let value = inner.value()?.parse::<syn::LitStr>()?.value();
                found |= inner.path.is_ident(direction) && value == "camelCase";
                Ok(())
            })
        });
    }
    found
}
