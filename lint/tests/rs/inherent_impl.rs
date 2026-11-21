use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::rs::inherent_impl::check;

#[test]
fn an_inherent_impl_in_the_defining_file_passes() {
    let source = "\
struct Tube;

impl Tube {
    fn new() -> Self {
        Self
    }
}

impl std::fmt::Debug for Other {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(\"other\")
    }
}
";
    assert!(with_source(Path::new("api/src/tube.rs"), source, &[], check).is_empty());
}

#[test]
fn an_inherent_impl_outside_the_defining_file_fails() {
    let source = "\
impl Tube {
    fn new() -> Self {
        Self
    }
}
";
    let found = with_source(Path::new("api/src/other.rs"), source, &[], check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "inherent-impl");
    assert_eq!(
        found[0].problem,
        "The inherent impl of `Tube` is outside the file that defines `Tube`."
    );
    assert_eq!(
        found[0].fix,
        "Move the inherent impl into the file that defines `Tube`."
    );
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/tube.rs"), "fn (", &[], check).is_empty());
}
