use std::path::{Path, PathBuf};
use std::{env, fs, process};

use ian_lint::files::FileSet;
use ian_lint::rule::folder::rs_test_tree::check;

fn tree(paths: &[&str]) -> FileSet {
    FileSet::from_paths(paths.iter().map(PathBuf::from))
}

#[test]
fn a_dot_src_path_still_checks_the_test_tree() {
    let files = tree(&["api/src/lib.rs", "api/src/a.rs", "api/src/tests/a.rs"]);
    let srcs = [PathBuf::from("./api/src")];
    let found = check(Path::new("api/src"), &files, &srcs);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].problem, "The `tests` folder has no `mod.rs`.");
    assert_eq!(found[0].path, PathBuf::from("api/src/tests"));
}

#[test]
fn a_matching_test_tree_passes() {
    let files = tree(&[
        "api/src/lib.rs",
        "api/src/a.rs",
        "api/src/b/mod.rs",
        "api/src/b/c.rs",
        "api/src/tests/mod.rs",
        "api/src/tests/a.rs",
        "api/src/tests/b/mod.rs",
        "api/src/tests/b/c.rs",
    ]);
    let srcs = [PathBuf::from("api/src")];
    assert!(check(Path::new("api/src"), &files, &srcs).is_empty());
}

#[test]
fn a_test_file_with_no_source_file_fails() {
    let files = tree(&[
        "api/src/lib.rs",
        "api/src/a.rs",
        "api/src/tests/mod.rs",
        "api/src/tests/a.rs",
        "api/src/tests/missing.rs",
        "api/src/tests/b/mod.rs",
    ]);
    let srcs = [PathBuf::from("api/src")];
    let found = check(Path::new("api/src"), &files, &srcs);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].rule, "test-tree");
    assert_eq!(
        found[0].problem,
        "The test file has no source file `api/src/b/mod.rs`."
    );
    assert_eq!(
        found[0].fix,
        "Give the test the same path as the source file."
    );
    assert_eq!(
        found[1].problem,
        "The test file has no source file `api/src/missing.rs`."
    );
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_test_folder_with_no_mod_rs_fails() {
    let files = tree(&[
        "api/src/lib.rs",
        "api/src/a.rs",
        "api/src/c/a.rs",
        "api/src/tests/a.rs",
        "api/src/tests/c/a.rs",
    ]);
    let srcs = [PathBuf::from("api/src")];
    let found = check(Path::new("api/src"), &files, &srcs);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].problem, "The `tests` folder has no `mod.rs`.");
    assert_eq!(
        found[0].fix,
        "Add a `mod.rs` that declares the child modules."
    );
    assert_eq!(found[0].path, PathBuf::from("api/src/tests"));
    assert_eq!(found[1].path, PathBuf::from("api/src/tests/c"));
    assert!(found.iter().all(|item| item.problem == found[0].problem));
}

#[test]
fn the_same_cfg_on_a_child_module_passes() {
    let root = temp_root("match");
    let src = root.join("api/src");
    write(
        &src,
        "lib.rs",
        "\
#[cfg(test)]
mod tests;

#[cfg(feature = \"auth\")]
pub mod auth;

#[cfg(all(feature = \"monitor\", target_os = \"linux\"))]
pub mod monitor;

pub mod time;
",
    );
    write(
        &src,
        "tests/mod.rs",
        "\
#[cfg(feature = \"auth\")]
mod auth;

#[cfg(all(feature = \"monitor\", target_os = \"linux\"))]
mod monitor;

mod time;
",
    );
    let files = FileSet::from_paths([src.join("lib.rs"), src.join("tests/mod.rs")]);
    let found = check(&src, &files, &[src.clone()]);
    fs::remove_dir_all(root).unwrap();
    assert!(found.is_empty());
}

#[test]
fn a_different_cfg_on_a_child_module_fails() {
    let root = temp_root("differ");
    let src = root.join("api/src");
    write(
        &src,
        "lib.rs",
        "\
#[cfg(feature = \"auth\")]
pub mod auth;
",
    );
    write(
        &src,
        "tests/mod.rs",
        "\
mod auth;
",
    );
    let files = FileSet::from_paths([src.join("lib.rs"), src.join("tests/mod.rs")]);
    let found = check(&src, &files, &[src.clone()]);
    fs::remove_dir_all(root).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "test-tree");
    assert_eq!(
        found[0].problem,
        "The cfg on `auth` does not match the source module."
    );
    assert_eq!(
        found[0].fix,
        "Use the same `#[cfg(...)]` as the source module."
    );
}

#[test]
fn the_test_root_follows_main_rs_when_lib_rs_is_absent() {
    let root = temp_root("main");
    let src = root.join("api/src");
    write(
        &src,
        "main.rs",
        "\
#[cfg(feature = \"auth\")]
pub mod auth;
",
    );
    write(
        &src,
        "tests/mod.rs",
        "\
#[cfg(feature = \"auth\")]
mod auth;
",
    );
    let files = FileSet::from_paths([src.join("main.rs"), src.join("tests/mod.rs")]);
    let found = check(&src, &files, &[src.clone()]);
    fs::remove_dir_all(root).unwrap();
    assert!(found.is_empty());
}

#[test]
fn a_different_cfg_against_main_rs_fails() {
    let root = temp_root("main-differ");
    let src = root.join("api/src");
    write(
        &src,
        "main.rs",
        "\
#[cfg(feature = \"auth\")]
pub mod auth;
",
    );
    write(
        &src,
        "tests/mod.rs",
        "\
mod auth;
",
    );
    let files = FileSet::from_paths([src.join("main.rs"), src.join("tests/mod.rs")]);
    let found = check(&src, &files, &[src.clone()]);
    fs::remove_dir_all(root).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].problem,
        "The cfg on `auth` does not match the source module."
    );
}

#[test]
fn lib_rs_stays_the_test_root_when_main_rs_is_also_present() {
    let root = temp_root("both");
    let src = root.join("api/src");
    write(
        &src,
        "lib.rs",
        "\
#[cfg(feature = \"auth\")]
pub mod auth;
",
    );
    write(&src, "main.rs", "pub mod auth;\n");
    write(&src, "tests/mod.rs", "mod auth;\n");
    let files = FileSet::from_paths([
        src.join("lib.rs"),
        src.join("main.rs"),
        src.join("tests/mod.rs"),
    ]);
    let found = check(&src, &files, &[src.clone()]);
    fs::remove_dir_all(root).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].problem,
        "The cfg on `auth` does not match the source module."
    );
}

fn temp_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("ian-lint-test-tree-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    fs::create_dir_all(&root).expect("test folder is created");
    root
}

fn write(src: &Path, path: &str, text: &str) {
    let path = src.join(path);
    fs::create_dir_all(path.parent().expect("path has a folder")).expect("folder is created");
    fs::write(path, text).expect("file is written");
}
