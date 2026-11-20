use std::path::{Path, PathBuf};

use crate::rule::rs::file::with_source;
use crate::rule::rs::test_attr::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

#[test]
fn a_test_in_the_test_folder_passes() {
    let source = "\
#[cfg(test)]
mod only_for_test {
    #[test]
    fn reads() {}

    #[tokio::test]
    async fn waits() {}
}

#[cfg(feature = \"test\")]
fn not_a_test_attr() {}

#[cfg(any(test, feature = \"x\"))]
fn not_only_test() {}
";
    assert!(with_source(Path::new("api/src/test/read.rs"), source, &srcs(), check).is_empty());
    let lib = "\
#[cfg(test)]
mod test;

fn run() {}
";
    assert!(with_source(Path::new("api/src/lib.rs"), lib, &srcs(), check).is_empty());
    assert!(with_source(Path::new("api/src/main.rs"), lib, &srcs(), check).is_empty());
}

#[test]
fn a_test_outside_the_test_folder_fails() {
    let source = "\
#[test]
fn reads() {}

#[tokio::test(flavor = \"multi_thread\")]
async fn waits() {}

#[cfg(test)]
fn hidden() {}

#[cfg(test)]
mod test {}
";
    let found = with_source(Path::new("api/src/read.rs"), source, &srcs(), check);
    assert_eq!(found.len(), 4);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "test-attr");
    assert_eq!(found[0].problem, "A test is outside the test folder.");
    assert_eq!(found[0].fix, "Move the test under the test folder.");
    assert_eq!(found[1].line, 4);
    assert_eq!(found[2].line, 7);
    assert_eq!(found[3].line, 10);
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/read.rs"), "fn (", &srcs(), check).is_empty());
}
