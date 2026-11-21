use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::declaration_order::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

#[test]
fn items_in_order_pass() {
    let source = "\
import { read } from \"pkg\";
import type { Kind } from \"pkg\";
type Local = string;
const value = 1;
let ready = false;
export const name = \"a\";
export type Public = string;
export function run(): void {}
interface Hidden {
  name: string;
}
function helper(): void {}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn an_item_out_of_order_fails() {
    let source = "\
import { read } from \"pkg\";
export const name = \"a\";
type Local = string;
const value = 1;
function helper(): void {}
interface Hidden {
  name: string;
}
";
    let found = diagnostics(source);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, vec![3, 4, 6]);
    assert_eq!(found[0].rule, "declaration-order");
    assert_eq!(found[0].problem, "This item is out of declaration order.");
    assert_eq!(
        found[0].fix,
        "Put imports first, then private types and constants, then exported items, then private interfaces, then private functions."
    );
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}
