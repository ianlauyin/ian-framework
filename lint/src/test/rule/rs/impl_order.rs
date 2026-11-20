use std::path::Path;

use crate::rule::edit::apply;
use crate::rule::rs::file::with_source;
use crate::rule::rs::impl_order::{check, edit};

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
    assert!(with_source(Path::new("api/src/tube.rs"), source, &[], check).is_empty());
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
    let found = with_source(Path::new("api/src/tube.rs"), source, &[], check);
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
    assert!(with_source(Path::new("api/src/tube.rs"), "fn (", &[], check).is_empty());
}

fn formatted(source: &str) -> String {
    with_source(Path::new("api/src/tube.rs"), source, &[], |file| {
        let found = edit(file);
        let refs: Vec<_> = found.iter().collect();
        apply(source, &refs).expect("edits apply")
    })
}

#[test]
fn an_inherent_impl_after_a_trait_impl_is_formatted() {
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
    let next = formatted(source);
    assert!(with_source(Path::new("api/src/tube.rs"), &next, &[], check).is_empty());
    assert!(next.find("impl Tube {").unwrap() < next.find("impl std::fmt::Debug").unwrap());
}

#[test]
fn the_edit_does_not_sort_the_source_order() {
    let source = "\
pub struct Name;

use std::fmt;

impl std::fmt::Debug for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(\"name\")
    }
}

impl Name {
    fn new() -> Self {
        Self
    }
}
";
    let next = formatted(source);
    assert!(with_source(Path::new("api/src/name.rs"), &next, &[], check).is_empty());
    assert!(next.find("pub struct Name;").unwrap() < next.find("use std::fmt;").unwrap());
    assert!(next.find("impl Name {").unwrap() < next.find("impl std::fmt::Debug").unwrap());
}
