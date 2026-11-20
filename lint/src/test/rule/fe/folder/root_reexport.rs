use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::files::FileSet;
use crate::rule::fe::folder::root_reexport::{check, check_source};

fn temp_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("ian-lint-root-reexport-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    fs::create_dir_all(&root).expect("test folder is created");
    root
}

#[test]
fn a_root_index_that_re_exports_each_feature_passes() {
    let source = "\
export * from \"./http\";
export { storage } from \"./storage.ts\";
export { view } from \"./view/index.tsx\";
";
    assert!(
        check_source(
            Path::new("app/src/index.ts"),
            source,
            &["http", "storage", "view"]
        )
        .is_empty()
    );
}

#[test]
fn a_missing_feature_fails() {
    let source = "export * from \"./http/ajax\";\n";
    let found = check_source(Path::new("app/src/index.ts"), source, &["http", "logger"]);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].rule, "root-reexport");
    assert_eq!(found[0].problem, "`./http` is missing from the index.");
    assert_eq!(found[0].fix, "Re-export `./http` from the index.");
    assert_eq!(found[1].problem, "`./logger` is missing from the index.");
    assert_eq!(found[1].fix, "Re-export `./logger` from the index.");
}

#[test]
fn a_root_on_disk_passes() {
    let root = temp_root("pass");
    let src = root.join("src");
    fs::create_dir_all(src.join("http")).expect("folder is created");
    fs::write(
        src.join("index.ts"),
        "export * from \"./http\";\nexport { storage } from \"./storage\";\n",
    )
    .expect("file is written");
    let files = FileSet::from_paths([
        src.join("index.ts"),
        src.join("http/index.ts"),
        src.join("storage.ts"),
    ]);
    let found = check(&src, &files, &[src.clone()]);
    fs::remove_dir_all(&root).expect("test folder is removed");
    assert!(found.is_empty());
}

#[test]
fn a_nested_folder_passes() {
    let root = temp_root("nested");
    let http = root.join("src/http");
    fs::create_dir_all(&http).expect("folder is created");
    fs::write(http.join("index.ts"), "export const local = 1;\n").expect("file is written");
    fs::write(http.join("ajax.ts"), "export const ajax = 1;\n").expect("file is written");
    let files = FileSet::from_paths([http.join("index.ts"), http.join("ajax.ts")]);
    let found = check(&http, &files, &[root.join("src")]);
    fs::remove_dir_all(&root).expect("test folder is removed");
    assert!(found.is_empty());
}

#[test]
fn a_src_path_with_a_leading_dot_finds_a_missing_feature() {
    let root = PathBuf::from(format!(
        "target/ian-lint-root-reexport-dot-{}",
        process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    let src = root.join("src");
    fs::create_dir_all(src.join("http")).expect("folder is created");
    fs::write(src.join("index.ts"), "export const local = 1;\n").expect("file is written");
    let files = FileSet::from_paths([src.join("index.ts"), src.join("http/index.ts")]);
    let found = check(&src, &files, &[Path::new(".").join(&src)]);
    fs::remove_dir_all(&root).expect("test folder is removed");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].problem, "`./http` is missing from the index.");
}
