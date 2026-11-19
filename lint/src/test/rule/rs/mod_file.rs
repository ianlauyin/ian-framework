use std::path::{Path, PathBuf};

use crate::rule::rs::mod_file::check_source;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

#[test]
fn a_mod_rs_with_the_folder_item_passes() {
    let source = "\
pub(crate) mod inner;

use std::fmt;

pub use inner::NAME;
pub(crate) use inner::hidden;

pub struct Logger;

impl Logger {
    pub fn new() -> Self {
        Self
    }
}

impl fmt::Debug for Logger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(\"logger\")
    }
}

const LABEL: &str = \"a\";

fn helper() {}
";
    assert!(check_source(Path::new("api/src/logger/mod.rs"), source, &srcs()).is_empty());
    let trait_source = "\
pub(crate) mod http_log_shipper;

pub trait LogShipper {}
";
    assert!(
        check_source(
            Path::new("api/src/log_shipper/mod.rs"),
            trait_source,
            &srcs()
        )
        .is_empty()
    );
    assert!(
        check_source(
            Path::new("api/src/test/logger/mod.rs"),
            "pub struct Other;\nfn helper() {}\n",
            &srcs()
        )
        .is_empty()
    );
}

#[test]
fn an_extra_item_in_mod_rs_fails() {
    let source = "\
pub struct Other;

fn helper() {}
";
    let found = check_source(Path::new("api/src/logger/mod.rs"), source, &srcs());
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "mod-file");
    assert_eq!(found[0].problem, "`Other` is not allowed in `mod.rs`.");
    assert_eq!(found[0].fix, "Move it out of `mod.rs`.");
    assert_eq!(found[1].line, 3);
    assert_eq!(found[1].problem, "`helper` is not allowed in `mod.rs`.");
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(check_source(Path::new("api/src/logger/mod.rs"), "fn (", &srcs()).is_empty());
}
