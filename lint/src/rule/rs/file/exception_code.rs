// The error code in `exception!` is a string literal.
// Pass a constant for the error code.
// Why a lint rule: The error code is a constant.

use std::collections::BTreeSet;

use proc_macro2::{Span, TokenStream, TokenTree};
use syn::visit::Visit;
use syn::{Expr, ExprLit, ExprMacro, ItemMacro, Lit};

use crate::parser::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::place::Place;
use crate::rule::rs::rule::RsRule;

const RULE: &str = "exception-code";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub(crate) fn check(file: &RsFile) -> Vec<Diagnostic> {
    if Place::new(file.path, file.srcs).in_folder("test") {
        return Vec::new();
    }
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut pass = Pass {
        report: &mut report,
        reported: BTreeSet::new(),
    };
    pass.visit_file(ast);
    report.finish()
}

struct Pass<'a, 'b> {
    report: &'b mut RuleReport<'a>,
    reported: BTreeSet<u32>,
}

impl Visit<'_> for Pass<'_, '_> {
    fn visit_expr_macro(&mut self, mac: &ExprMacro) {
        if is_exception(&mac.mac.path) {
            self.scan(mac.mac.tokens.clone());
        }
        syn::visit::visit_expr_macro(self, mac);
    }

    fn visit_item_macro(&mut self, mac: &ItemMacro) {
        if mac.ident.is_none() && is_exception(&mac.mac.path) {
            self.scan(mac.mac.tokens.clone());
        }
        syn::visit::visit_item_macro(self, mac);
    }
}

impl Pass<'_, '_> {
    fn scan(&mut self, tokens: TokenStream) {
        let list: Vec<_> = tokens.into_iter().collect();
        let mut index = 0;
        while index < list.len() {
            if is_exception_group(&list, index) {
                if let TokenTree::Group(group) = &list[index + 2] {
                    self.scan(group.stream());
                }
                index += 3;
                continue;
            }
            if is_ident(list.get(index), "code") && is_punct(list.get(index + 1), '=') {
                if let Some(span) = string_expr(&list[index + 2..]) {
                    self.report(span);
                }
            }
            if let TokenTree::Group(group) = &list[index] {
                self.scan(group.stream());
            }
            index += 1;
        }
    }

    fn report(&mut self, span: Span) {
        let offset = span.byte_range().start as u32;
        if !self.reported.insert(offset) {
            return;
        }
        self.report.at_span(
            span,
            "The error code in `exception!` is a string literal.",
            "Pass a constant for the error code.",
        );
    }
}

fn is_exception(path: &syn::Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "exception")
}

fn is_exception_group(list: &[TokenTree], index: usize) -> bool {
    is_ident(list.get(index), "exception")
        && is_punct(list.get(index + 1), '!')
        && matches!(list.get(index + 2), Some(TokenTree::Group(_)))
}

fn is_ident(tree: Option<&TokenTree>, name: &str) -> bool {
    matches!(tree, Some(TokenTree::Ident(ident)) if ident == name)
}

fn is_punct(tree: Option<&TokenTree>, ch: char) -> bool {
    matches!(tree, Some(TokenTree::Punct(punct)) if punct.as_char() == ch)
}

fn string_expr(tokens: &[TokenTree]) -> Option<Span> {
    let mut expr_tokens = Vec::new();
    for tree in tokens {
        if matches!(tree, TokenTree::Punct(punct) if punct.as_char() == ',') {
            break;
        }
        expr_tokens.push(tree.clone());
    }
    let expr = syn::parse2::<Expr>(TokenStream::from_iter(expr_tokens)).ok()?;
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Some(value.span()),
        _ => None,
    }
}
