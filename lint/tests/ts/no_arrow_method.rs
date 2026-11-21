use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::no_arrow_method::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

#[test]
fn a_class_method_passes() {
    let source = "\
class A extends PageContext<State> {
    load() {}
    count = 1;
    helper = function () {};
}
class B {
    run = () => {};
}
class C extends Other {
    run = () => {};
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn an_arrow_method_fails() {
    let source = "\
export class A extends PageContext<State> {
    load = () => {};
}
export abstract class B extends Module<State> {
    run = async () => {};
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].rule, "no-arrow-method");
    assert_eq!(found[0].problem, "`load` is an arrow function.");
    assert_eq!(found[0].fix, "Write the method as a class method.");
    assert_eq!(found[1].line, 5);
    assert_eq!(found[1].problem, "`run` is an arrow function.");
}
