use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::override_method_order::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

#[test]
fn override_methods_between_the_constructor_and_the_other_methods_pass() {
    let source = "\
class A extends B {
    override count = 1;
    constructor() { super(); }
    override save() {}
    override get value() { return 1; }
    load() {}
    private helper() {}
}
class C extends B {
    override save() {}
    load() {}
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn an_override_method_before_the_constructor_fails() {
    let source = "\
export class A extends B {
    override save() {}
    override get value() { return 1; }
    constructor() { super(); }
}
class C {
    run() {
        return class extends B {
            override inner() {}
            constructor() { super(); }
        };
    }
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].rule, "override-method-order");
    assert_eq!(
        found[0].problem,
        "The override method `save` is before the constructor."
    );
    assert_eq!(
        found[0].fix,
        "Move the override methods below the constructor and above the other methods of the class."
    );
    assert_eq!(found[1].line, 3);
    assert_eq!(
        found[1].problem,
        "The override method `value` is before the constructor."
    );
    assert_eq!(found[2].line, 9);
}

#[test]
fn an_override_method_after_another_method_fails() {
    let source = "\
class A extends B {
    load() {}
    constructor() { super(); }
    override save() {}
}
class C extends B {
    load() {}
    override save() {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 4);
    assert_eq!(
        found[0].problem,
        "The override method `save` is after a method that is not an override."
    );
    assert_eq!(found[1].line, 8);
}
