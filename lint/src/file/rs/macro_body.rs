use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Block, Expr, Macro, Stmt, Token};

pub(crate) fn stmts(mac: &Macro) -> Vec<Stmt> {
    let exprs = Punctuated::<Expr, Token![,]>::parse_terminated.parse2(mac.tokens.clone());
    if let Ok(exprs) = exprs {
        return exprs
            .into_iter()
            .map(|expr| Stmt::Expr(expr, None))
            .collect();
    }
    Block::parse_within
        .parse2(mac.tokens.clone())
        .unwrap_or_default()
}
