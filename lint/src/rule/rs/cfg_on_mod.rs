// `#[cfg(target_os)]` is not on a `mod` line.
// Put the attribute on the `mod` line of the module.
// `main.rs` has more than one `#[cfg(target_os)]` block.
// Start all the code for that target in one block.
// Why a lint rule: The code for one target stays in its module, and `main.rs` starts it in one place.

use proc_macro2::{TokenStream, TokenTree};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, ExprBlock, ItemMod, Meta};

use crate::file::place::Place;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::rs::RsRule;

const RULE: &str = "cfg-on-mod";

pub(crate) const RS_RULE: RsRule = RsRule::file(check);

pub fn check(file: &RsFile) -> Vec<Diagnostic> {
    let Some(ast) = file.ast else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut pass = Pass {
        report: &mut report,
        main: Place::new(file.path, file.srcs).is_file("main.rs"),
        blocks: 0,
    };
    pass.visit_file(ast);
    report.finish()
}

struct Pass<'a, 'b> {
    report: &'b mut RuleReport<'a>,
    main: bool,
    blocks: u32,
}

impl Visit<'_> for Pass<'_, '_> {
    fn visit_attribute(&mut self, attr: &Attribute) {
        if is_target_cfg(attr) {
            self.report.at_span(
                attr.span(),
                "`#[cfg(target_os)]` is not on a `mod` line.",
                "Put the attribute on the `mod` line of the module.",
            );
        }
    }

    fn visit_item_mod(&mut self, module: &ItemMod) {
        if let Some((_, items)) = &module.content {
            for item in items {
                self.visit_item(item);
            }
        }
    }

    fn visit_expr_block(&mut self, block: &ExprBlock) {
        for attr in &block.attrs {
            if !self.main || !is_target_cfg(attr) {
                self.visit_attribute(attr);
                continue;
            }
            self.blocks += 1;
            if self.blocks > 1 {
                self.report.at_span(
                    attr.span(),
                    "`main.rs` has more than one `#[cfg(target_os)]` block.",
                    "Start all the code for that target in one block.",
                );
            }
        }
        self.visit_block(&block.block);
    }
}

fn is_target_cfg(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    match &attr.meta {
        Meta::List(list) => names_target(list.tokens.clone()),
        _ => false,
    }
}

fn names_target(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|tree| match tree {
        TokenTree::Ident(ident) => ident == "target_os",
        TokenTree::Group(group) => names_target(group.stream()),
        _ => false,
    })
}
