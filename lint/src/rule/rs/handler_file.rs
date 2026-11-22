// The handler `{name}` does not have the name of its file.
// Give the handler the name of the file, or move the handler to its own file.
// The handler `{name}` is not `pub(super)`.
// Make the handler `pub(super)`.
// The handler `{name}` is in `mod.rs`.
// Move the handler to its own file.
// Why a lint rule: Each handler is in its own file, and only the router of its folder uses it.

use syn::{Item, ItemFn, Visibility};

use crate::file::place::Place;
use crate::file::rs::source::RsFile;
use crate::files::file_name;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "handler-file";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let place = Place::new(file.path, file.srcs);
    let parts = place.parts();
    let Some((name, folders)) = parts.split_last() else {
        return Vec::new();
    };
    if place.in_folder("tests") || !folders.contains(&"router") {
        return Vec::new();
    }
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let stem = name.strip_suffix(".rs").unwrap_or(name);
    let mut report = RuleReport::new(file.path, file.source, RULE);
    for item in &ast.items {
        let Item::Fn(item) = item else {
            continue;
        };
        if item.sig.asyncness.is_none() || matches!(item.vis, Visibility::Inherited) {
            continue;
        }
        report_handler(item, stem, file_name(file.path) == "mod.rs", &mut report);
    }
    report.finish()
}

fn report_handler(item: &ItemFn, stem: &str, mod_file: bool, report: &mut RuleReport<'_>) {
    let name = &item.sig.ident;
    if mod_file {
        report.at_span(
            name.span(),
            format!("The handler `{name}` is in `mod.rs`."),
            "Move the handler to its own file.",
        );
        return;
    }
    if name != stem {
        report.at_span(
            name.span(),
            format!("The handler `{name}` does not have the name of its file."),
            "Give the handler the name of the file, or move the handler to its own file.",
        );
    }
    if !is_pub_super(&item.vis) {
        report.at_span(
            name.span(),
            format!("The handler `{name}` is not `pub(super)`."),
            "Make the handler `pub(super)`.",
        );
    }
}

fn is_pub_super(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Restricted(vis) if vis.in_token.is_none() && vis.path.is_ident("super"))
}
