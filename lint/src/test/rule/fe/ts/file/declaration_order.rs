use std::path::Path;

use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::ts::file::declaration_order::{check, edit};
use crate::test::rule::fe::ts::source::with_source;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

fn formatted(source: &str) -> String {
    with_source(Path::new("src/a.ts"), source, |file| {
        let found = edit(file);
        let edits: Vec<_> = found.iter().collect();
        crate::rule::edit::apply(source, &edits).expect("edits apply")
    })
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

#[test]
fn an_item_out_of_order_is_formatted() {
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
    let next = formatted(source);
    assert!(diagnostics(&next).is_empty());
    let import_at = next.find("import").unwrap();
    let type_at = next.find("type Local").unwrap();
    let const_at = next.find("const value").unwrap();
    let export_at = next.find("export const").unwrap();
    let interface_at = next.find("interface").unwrap();
    let function_at = next.find("function").unwrap();
    assert!(import_at < type_at);
    assert!(type_at < const_at);
    assert!(const_at < export_at);
    assert!(export_at < interface_at);
    assert!(interface_at < function_at);
}

#[test]
fn items_in_order_are_unchanged() {
    let source = "\
import { read } from \"pkg\";
type Local = string;
const value = 1;
export const name = \"a\";
interface Hidden {
  name: string;
}
function helper(): void {}
";
    assert_eq!(formatted(source), source);
}

#[test]
fn a_class_is_not_formatted() {
    let source = "\
export const name = \"a\";
class Name {}
";
    assert_eq!(formatted(source), source);
}
