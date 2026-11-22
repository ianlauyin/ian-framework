use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::interface_object_shape::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

#[test]
fn an_interface_and_a_type_that_is_not_an_object_shape_pass() {
    let source = "\
export interface Store {
    count: number;
}
type Theme = \"light\" | \"dark\";
type Id = string;
type Keys = keyof Store;
type Flags = {[K in Keys]: boolean};
type Pair = [number, number];
type Run = () => void;
type Wide = Store & {name: string};
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_type_that_is_an_object_shape_fails() {
    let source = "\
type Store = {
    count: number;
};
export type Options = {};
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "interface-object-shape");
    assert_eq!(found[0].problem, "The type `Store` is an object shape.");
    assert_eq!(found[0].fix, "Declare the object shape with `interface`.");
    assert_eq!(found[1].line, 4);
    assert_eq!(found[1].problem, "The type `Options` is an object shape.");
}
