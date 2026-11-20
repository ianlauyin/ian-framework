// `{name}` is not allowed in `mod.rs`.
// Move it out of `mod.rs`.
// Why a lint rule: A `mod.rs` holds module declarations and `pub use` re-exports, plus the item named after the folder and that item's private items and impl blocks.

use syn::spanned::Spanned;
use syn::{Item, Type, Visibility};

use crate::diagnostic::Diagnostic;
use crate::files::file_name;
use crate::rule::report::Report;
use crate::rule::rs::file::RsFile;
use crate::rule::rs::place::Place;

const RULE: &str = "mod-file";

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    if file_name(file.path) != "mod.rs" || Place::new(file.path, file.srcs).in_test() {
        return Vec::new();
    }
    let Some(folder) = file.path.parent().and_then(|parent| parent.file_name()) else {
        return Vec::new();
    };
    let Some(folder) = folder.to_str() else {
        return Vec::new();
    };
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let pascal = pascal_case(folder);
    let has_folder_item = ast
        .items
        .iter()
        .any(|item| is_folder_item(item, &pascal, folder));
    let mut report = Report::new(file.path, file.source, RULE);
    for item in &ast.items {
        if is_allowed(item, &pascal, folder, has_folder_item) {
            continue;
        }
        report.at_span(
            item.span(),
            format!("`{}` is not allowed in `mod.rs`.", item_label(item)),
            "Move it out of `mod.rs`.",
        );
    }
    report.finish()
}

fn is_allowed(item: &Item, pascal: &str, snake: &str, has_folder_item: bool) -> bool {
    if matches!(item, Item::Mod(_) | Item::Use(_)) {
        return true;
    }
    if is_folder_item(item, pascal, snake) || is_impl_of(item, pascal) {
        return true;
    }
    has_folder_item && is_private_item(item)
}

fn is_folder_item(item: &Item, pascal: &str, snake: &str) -> bool {
    match item {
        Item::Struct(item) => item.ident == pascal,
        Item::Enum(item) => item.ident == pascal,
        Item::Union(item) => item.ident == pascal,
        Item::Trait(item) => item.ident == pascal,
        Item::Type(item) => item.ident == pascal,
        Item::Fn(item) => item.sig.ident == snake,
        Item::Const(item) => item.ident == snake,
        _ => false,
    }
}

fn is_impl_of(item: &Item, pascal: &str) -> bool {
    let Item::Impl(item) = item else {
        return false;
    };
    type_name(&item.self_ty).is_some_and(|name| name == pascal)
}

fn is_private_item(item: &Item) -> bool {
    match item {
        Item::Macro(item) => !item
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("macro_export")),
        Item::Impl(_) => false,
        _ => vis_of(item).is_some_and(|vis| matches!(vis, Visibility::Inherited)),
    }
}

fn item_label(item: &Item) -> String {
    match item {
        Item::Const(item) => item.ident.to_string(),
        Item::Enum(item) => item.ident.to_string(),
        Item::Fn(item) => item.sig.ident.to_string(),
        Item::Struct(item) => item.ident.to_string(),
        Item::Trait(item) => item.ident.to_string(),
        Item::Type(item) => item.ident.to_string(),
        Item::Union(item) => item.ident.to_string(),
        Item::Static(item) => item.ident.to_string(),
        Item::Impl(item) => type_name(&item.self_ty)
            .map(|name| format!("impl {name}"))
            .unwrap_or_else(|| "impl".to_string()),
        _ => "This item".to_string(),
    }
}

fn type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Type::Group(ty) => type_name(&ty.elem),
        Type::Paren(ty) => type_name(&ty.elem),
        _ => None,
    }
}

fn vis_of(item: &Item) -> Option<&Visibility> {
    match item {
        Item::Const(item) => Some(&item.vis),
        Item::Enum(item) => Some(&item.vis),
        Item::ExternCrate(item) => Some(&item.vis),
        Item::Fn(item) => Some(&item.vis),
        Item::Mod(item) => Some(&item.vis),
        Item::Static(item) => Some(&item.vis),
        Item::Struct(item) => Some(&item.vis),
        Item::Trait(item) => Some(&item.vis),
        Item::TraitAlias(item) => Some(&item.vis),
        Item::Type(item) => Some(&item.vis),
        Item::Union(item) => Some(&item.vis),
        Item::Use(item) => Some(&item.vis),
        _ => None,
    }
}

fn pascal_case(name: &str) -> String {
    let mut out = String::new();
    for part in name.split('_') {
        let mut chars = part.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
    out
}
