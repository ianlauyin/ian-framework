use std::path::{Path, PathBuf};

use crate::rule::rs::place::Place;

#[test]
fn a_dot_src_path_matches_a_file_without_the_dot() {
    let srcs = [PathBuf::from("./api/src")];
    let env = Place::new(Path::new("api/src/env.rs"), &srcs);
    assert!(env.is_file("env.rs"));
    assert!(!env.in_test());
    assert!(!env.in_gcp());
    let test = Place::new(Path::new("api/src/test/read.rs"), &srcs);
    assert!(test.in_test());
    assert!(!test.is_file("read.rs"));
    let gcp = Place::new(Path::new("api/src/gcp/project.rs"), &srcs);
    assert!(gcp.in_gcp());
    let lib = Place::new(Path::new("api/src/lib.rs"), &srcs);
    assert!(lib.is_file("lib.rs"));
    let root_test = Place::new(Path::new("api/src/test.rs"), &srcs);
    assert!(!root_test.in_test());
}
