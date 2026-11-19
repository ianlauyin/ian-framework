use std::path::Path;

use crate::rule::rs::impl_order::check_source;

#[test]
fn an_inherent_impl_before_a_trait_impl_passes() {
    let source = "\
pub struct Tube;

impl Tube {
    fn new() -> Self {
        Self
    }
}

impl std::fmt::Debug for Tube {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(\"tube\")
    }
}

impl std::fmt::Display for Tube {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(\"tube\")
    }
}

struct Hidden;

impl std::fmt::Debug for Hidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(\"hidden\")
    }
}

impl Hidden {
    fn new() -> Self {
        Self
    }
}
";
    assert!(check_source(Path::new("api/src/tube.rs"), source).is_empty());
}

#[test]
fn an_inherent_impl_after_a_trait_impl_fails() {
    let source = "\
pub struct Tube;

impl std::fmt::Debug for Tube {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(\"tube\")
    }
}

impl Tube {
    fn new() -> Self {
        Self
    }
}
";
    let found = check_source(Path::new("api/src/tube.rs"), source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 9);
    assert_eq!(found[0].rule, "impl-order");
    assert_eq!(
        found[0].problem,
        "The inherent impl of `Tube` comes after a trait impl."
    );
    assert_eq!(
        found[0].fix,
        "Put the inherent impl before the trait impls."
    );
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(check_source(Path::new("api/src/tube.rs"), "fn (").is_empty());
}
