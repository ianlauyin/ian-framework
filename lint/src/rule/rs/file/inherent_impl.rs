// The inherent impl of `{name}` is outside the file that defines `{name}`.
// Move the inherent impl into the file that defines `{name}`.
// Why a lint rule: The type and its methods stay in one file. A trait impl can be in another file.

use std::collections::BTreeSet;

use syn::spanned::Spanned;
use syn::{Item, Type};

use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::rule::RsRule;
use crate::rule::rs::source::RsFile;

const RULE: &str = "inherent-impl";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub(crate) fn check(file: &RsFile) -> Vec<Diagnostic> {
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut defined = BTreeSet::new();
    collect_types(&ast.items, &mut defined);
    walk(&ast.items, &defined, &mut report);
    report.finish()
}

fn collect_types(items: &[Item], defined: &mut BTreeSet<String>) {
    for item in items {
        match item {
            Item::Struct(item) => {
                defined.insert(item.ident.to_string());
            }
            Item::Enum(item) => {
                defined.insert(item.ident.to_string());
            }
            Item::Union(item) => {
                defined.insert(item.ident.to_string());
            }
            Item::Mod(item) => {
                if let Some((_, nested)) = &item.content {
                    collect_types(nested, defined);
                }
            }
            _ => {}
        }
    }
}

fn walk(items: &[Item], defined: &BTreeSet<String>, report: &mut RuleReport<'_>) {
    for item in items {
        if let Item::Impl(item) = item
            && item.trait_.is_none()
            && let Some(name) = type_name(&item.self_ty)
            && !defined.contains(&name)
        {
            report.at_span(
                item.span(),
                format!("The inherent impl of `{name}` is outside the file that defines `{name}`."),
                format!("Move the inherent impl into the file that defines `{name}`."),
            );
        }
        if let Item::Mod(item) = item
            && let Some((_, nested)) = &item.content
        {
            walk(nested, defined, report);
        }
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
