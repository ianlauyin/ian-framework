use std::path::Path;

use crate::rule::rs::file::with_source;
use crate::rule::rs::reexport_mod::check;

#[test]
fn a_reexported_child_module_is_pub_crate_passes() {
    let source = "\
pub(crate) mod child;
mod other;

pub use child::Name;
pub use super::error::Code;
pub(crate) use other::Hidden;
";
    assert!(with_source(Path::new("api/src/parent.rs"), source, &[], check).is_empty());
}

#[test]
fn a_reexported_child_module_that_is_not_pub_crate_fails() {
    let source = "\
mod child;
pub mod open;

pub use child::Name;
pub use open::Other;
";
    let found = with_source(Path::new("api/src/parent.rs"), source, &[], check);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "reexport-mod");
    assert_eq!(
        found[0].problem,
        "`child` is re-exported and is not `pub(crate) mod`."
    );
    assert_eq!(found[0].fix, "Declare `child` as `pub(crate) mod`.");
    assert_eq!(found[1].line, 2);
    assert_eq!(
        found[1].problem,
        "`open` is re-exported and is not `pub(crate) mod`."
    );
    assert!(found.iter().all(|item| item.fix.starts_with("Declare ")));
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/parent.rs"), "fn (", &[], check).is_empty());
}
