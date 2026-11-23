use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::private_method_order::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

#[test]
fn private_methods_last_passes() {
    let source = "\
class A {
    private count = 1;
    constructor() {}
    load() {}
    protected save() {}
    static make() {}
    private helper() {}
    #other() {}
}
class B {
    private helper() {}
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_private_method_before_a_method_fails() {
    let source = "\
export class A {
    private helper() {}
    load() {}
    #other() {}
    get value() { return 1; }
    private last() {}
}
class B {
    run() {
        return class {
            private inner() {}
            outer() {}
        };
    }
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].rule, "private-method-order");
    assert_eq!(
        found[0].problem,
        "The private method `helper` is before a method that is not private."
    );
    assert_eq!(
        found[0].fix,
        "Move the private methods below all the other methods of the class."
    );
    assert_eq!(found[1].line, 4);
    assert_eq!(
        found[1].problem,
        "The private method `#other` is before a method that is not private."
    );
    assert_eq!(found[2].line, 11);
}
