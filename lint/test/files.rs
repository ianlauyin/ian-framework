use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::files::{FileSet, is_frontend_source, is_rust_source, is_style_file};

fn temp_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("ian-lint-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    fs::create_dir_all(&root).expect("test folder is created");
    root
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("path has a folder")).expect("folder is created");
    fs::write(path, text).expect("file is written");
}

#[test]
fn a_source_file_has_one_kind() {
    let fe = [PathBuf::from("web/src")];
    let be = [PathBuf::from("api/src")];
    assert!(is_frontend_source(Path::new("web/src/app.ts"), &fe));
    assert!(is_frontend_source(Path::new("web/src/app.tsx"), &fe));
    assert!(is_frontend_source(Path::new("web/src/app.css"), &fe));
    assert!(is_style_file(Path::new("web/src/app.css"), &fe));
    assert!(!is_style_file(Path::new("web/src/app.ts"), &fe));
    assert!(!is_style_file(Path::new("other/src/app.css"), &fe));
    assert!(!is_frontend_source(Path::new("web/src/app.d.ts"), &fe));
    assert!(!is_frontend_source(Path::new("web/src/app.rs"), &fe));
    assert!(!is_frontend_source(Path::new("other/src/app.ts"), &fe));
    assert!(!is_frontend_source(Path::new("other/src/app.css"), &fe));
    assert!(is_rust_source(Path::new("api/src/app.rs"), &be));
    assert!(!is_rust_source(Path::new("api/src/app.ts"), &be));
    assert!(!is_rust_source(Path::new("api/tests/app.rs"), &be));
    assert!(!is_rust_source(Path::new("web/src/app.rs"), &be));
}

#[test]
fn walk_records_a_folder_that_has_no_file() {
    let root = temp_root("empty-folder");
    write(&root, "src/app.ts", "const a = 1;\n");
    fs::create_dir_all(root.join("src/empty")).expect("folder is created");

    let (files, errors) = FileSet::walk(&root);
    assert!(errors.is_empty());
    assert!(files.has_dir(&root.join("src/empty")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn walk_obeys_gitignore_without_a_git_folder() {
    let root = temp_root("gitignore");
    write(&root, ".gitignore", "node_modules/\n");
    write(&root, "node_modules/pkg/src/a.ts", "const a = 1;\n");
    write(&root, "src/util/x.ts", "const x = 1;\n");

    let (files, errors) = FileSet::walk(&root);
    assert!(errors.is_empty());
    let paths: Vec<_> = files.files().collect();
    assert!(paths.iter().any(|path| path.ends_with("src/util/x.ts")));
    assert!(paths.iter().all(|path| {
        !path
            .components()
            .any(|part| part.as_os_str() == "node_modules")
    }));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn walk_gives_the_errors_of_a_root_that_it_cannot_read() {
    let parent = temp_root("missing");
    let root = parent.join("does-not-exist");
    let (files, errors) = FileSet::walk(&root);
    assert_eq!(files.files().count(), 0);
    assert_eq!(errors.len(), 1);
    fs::remove_dir_all(parent).unwrap();
}
