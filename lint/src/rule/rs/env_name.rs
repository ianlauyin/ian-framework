// An env value is read outside `env.rs`.
// Read the env value through `env.rs`.
// The env name `{name}` is not allowed.
// Let the caller pass the name.
// Use `GOOGLE_CLOUD_PROJECT` under `gcp`, or let the caller pass the name.
// Why a lint rule: The library does not choose an app env name.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Expr, ExprCall, ExprLit, ExprMacro, Ident, Item, Lit, UseTree};

use crate::file::rs::place::Place;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "env-name";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);
pub(crate) const FRAMEWORK_RS_RULE: RsRule = RsRule::file(check_framework);
const READS: &[&str] = &["var", "var_os", "vars", "vars_os"];
const HELPERS: &[&str] = &["optional", "required"];
const GOOGLE: &[&str] = &["GOOGLE_CLOUD_PROJECT"];
const FIX: &str = "Let the caller pass the name.";
const FRAMEWORK_FIX: &str =
    "Use `GOOGLE_CLOUD_PROJECT` under `gcp`, or let the caller pass the name.";

pub(crate) fn check(file: &RsFile) -> Vec<Diagnostic> {
    check_file(file, false)
}

pub(crate) fn check_framework(file: &RsFile) -> Vec<Diagnostic> {
    check_file(file, true)
}

fn check_file(file: &RsFile, allow_google: bool) -> Vec<Diagnostic> {
    let place = Place::new(file.path, file.srcs);
    if place.in_folder("test") {
        return Vec::new();
    }
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut pass = Pass {
        report: &mut report,
        place,
        allow_google,
        consts: const_values(&ast.items),
        reads: BTreeSet::new(),
        helpers: BTreeSet::new(),
        reported: BTreeSet::new(),
    };
    collect_uses(&ast.items, &mut pass.reads, &mut pass.helpers);
    pass.visit_file(ast);
    report.finish()
}

struct Pass<'a, 'b> {
    report: &'b mut RuleReport<'a>,
    place: Place,
    allow_google: bool,
    consts: BTreeMap<String, (String, Span)>,
    reads: BTreeSet<String>,
    helpers: BTreeSet<String>,
    reported: BTreeSet<u32>,
}

impl Visit<'_> for Pass<'_, '_> {
    fn visit_expr_call(&mut self, call: &ExprCall) {
        if let Expr::Path(path) = &*call.func {
            match kind(&path.path, &self.reads, &self.helpers) {
                Kind::Read => self.report_read(call.span()),
                Kind::Helper => self.report_name(call.args.first()),
                Kind::Other => {}
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_macro(&mut self, mac: &ExprMacro) {
        let segments: Vec<_> = mac
            .mac
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        if segments.len() == 1 && (segments[0] == "env" || segments[0] == "option_env") {
            self.report_read(mac.span());
        }
        syn::visit::visit_expr_macro(self, mac);
    }
}

enum Kind {
    Read,
    Helper,
    Other,
}

fn kind(path: &syn::Path, reads: &BTreeSet<String>, helpers: &BTreeSet<String>) -> Kind {
    let segments: Vec<_> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    if segments.len() >= 2 {
        let function = segments[segments.len() - 1].as_str();
        let previous = segments[segments.len() - 2].as_str();
        if previous == "env" && READS.contains(&function) {
            return Kind::Read;
        }
        if previous == "env" && HELPERS.contains(&function) {
            return Kind::Helper;
        }
    }
    if segments.len() == 1 && path.leading_colon.is_none() {
        if reads.contains(&segments[0]) {
            return Kind::Read;
        }
        if helpers.contains(&segments[0]) {
            return Kind::Helper;
        }
    }
    Kind::Other
}

impl Pass<'_, '_> {
    fn report_read(&mut self, span: Span) {
        if self.place.is_file("env.rs") {
            return;
        }
        self.report(
            span,
            "An env value is read outside `env.rs`.",
            "Read the env value through `env.rs`.",
        );
    }

    fn report_name(&mut self, arg: Option<&Expr>) {
        let Some(arg) = arg else {
            return;
        };
        let Some((name, span)) = literal(arg, &self.consts) else {
            return;
        };
        if self.allow_google && self.place.in_folder("gcp") && GOOGLE.contains(&name.as_str()) {
            return;
        }
        let fix = if self.allow_google {
            FRAMEWORK_FIX
        } else {
            FIX
        };
        self.report(span, format!("The env name `{name}` is not allowed."), fix);
    }

    fn report(&mut self, span: Span, problem: impl Into<String>, fix: impl Into<String>) {
        let offset = span.byte_range().start as u32;
        if !self.reported.insert(offset) {
            return;
        }
        self.report.at_span(span, problem, fix);
    }
}

fn literal(expr: &Expr, consts: &BTreeMap<String, (String, Span)>) -> Option<(String, Span)> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Some((value.value(), value.span())),
        Expr::Path(path)
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1 =>
        {
            let name = path.path.segments[0].ident.to_string();
            consts
                .get(&name)
                .map(|(value, span)| (value.clone(), *span))
        }
        _ => None,
    }
}

fn const_values(items: &[Item]) -> BTreeMap<String, (String, Span)> {
    let mut values = BTreeMap::new();
    collect_consts(items, &mut values);
    values
}

fn collect_consts(items: &[Item], values: &mut BTreeMap<String, (String, Span)>) {
    for item in items {
        match item {
            Item::Const(item) => insert_const(&item.ident, &item.expr, values),
            Item::Static(item) => insert_const(&item.ident, &item.expr, values),
            Item::Mod(item) => {
                if let Some((_, nested)) = &item.content {
                    collect_consts(nested, values);
                }
            }
            _ => {}
        }
    }
}

fn insert_const(ident: &Ident, expr: &Expr, values: &mut BTreeMap<String, (String, Span)>) {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(value),
        ..
    }) = expr
    else {
        return;
    };
    values.insert(ident.to_string(), (value.value(), value.span()));
}

fn collect_uses(items: &[Item], reads: &mut BTreeSet<String>, helpers: &mut BTreeSet<String>) {
    for item in items {
        match item {
            Item::Use(item) => collect_tree(&item.tree, &[], reads, helpers),
            Item::Mod(item) => {
                if let Some((_, nested)) = &item.content {
                    collect_uses(nested, reads, helpers);
                }
            }
            _ => {}
        }
    }
}

fn collect_tree(
    tree: &UseTree,
    prefix: &[String],
    reads: &mut BTreeSet<String>,
    helpers: &mut BTreeSet<String>,
) {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            collect_tree(&path.tree, &next, reads, helpers);
        }
        UseTree::Name(name) => bind(&name.ident, &name.ident, prefix, reads, helpers),
        UseTree::Rename(name) => bind(&name.ident, &name.rename, prefix, reads, helpers),
        UseTree::Glob(_) => {
            if is_std_env(prefix) {
                for name in READS {
                    reads.insert((*name).to_string());
                }
            }
            if is_helper_env(prefix) {
                for name in HELPERS {
                    helpers.insert((*name).to_string());
                }
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_tree(item, prefix, reads, helpers);
            }
        }
    }
}

fn bind(
    original: &Ident,
    local: &Ident,
    prefix: &[String],
    reads: &mut BTreeSet<String>,
    helpers: &mut BTreeSet<String>,
) {
    let name = original.to_string();
    if is_std_env(prefix) && READS.contains(&name.as_str()) {
        reads.insert(local.to_string());
    }
    if is_helper_env(prefix) && HELPERS.contains(&name.as_str()) {
        helpers.insert(local.to_string());
    }
}

fn is_std_env(prefix: &[String]) -> bool {
    prefix == ["std", "env"]
}

fn is_helper_env(prefix: &[String]) -> bool {
    prefix == ["env"] || prefix == ["crate", "env"] || prefix == ["self", "env"]
}
