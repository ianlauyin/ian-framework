use std::path::{Path, PathBuf};
use std::{env, fs, process};

use ian_lint::diagnostic::Tool;
use ian_lint::files::{FileSet, is_frontend_source, is_rust_source};

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
    assert!(is_frontend_source(Path::new("src/app.ts")));
    assert!(is_frontend_source(Path::new("src/app.tsx")));
    assert!(is_frontend_source(Path::new("src/app.css")));
    assert!(!is_frontend_source(Path::new("src/app.d.ts")));
    assert!(!is_frontend_source(Path::new("src/app.rs")));
    assert!(is_rust_source(Path::new("src/app.rs")));
    assert!(!is_rust_source(Path::new("src/app.ts")));
    assert!(!is_rust_source(Path::new("tests/app.rs")));
}

#[test]
fn module_name_is_the_nearest_folder_with_a_module_tsx() {
    let files = FileSet::from_paths(
        [
            "src/module/account/module.tsx",
            "src/module/account/billing/module.tsx",
            "src/module/account/billing/Form/index.css",
            "src/module/account/Panel/index.css",
            "src/component/Button/index.css",
        ]
        .map(PathBuf::from),
    );
    assert_eq!(
        files.module_name(Path::new("src/module/account/billing/Form/index.css")),
        Some("billing")
    );
    assert_eq!(
        files.module_name(Path::new("src/module/account/Panel/index.css")),
        Some("account")
    );
    assert_eq!(
        files.module_name(Path::new("src/component/Button/index.css")),
        None
    );
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
    let root = temp_root("missing").join("does-not-exist");
    let (files, errors) = FileSet::walk(&root);
    assert_eq!(files.files().count(), 0);
    assert_eq!(errors.len(), 1);
}

#[test]
fn lint_reports_no_diagnostics_when_there_are_no_rules() {
    let root = temp_root("empty-rules");
    write(&root, "src/util/x.ts", "const x = 1;\n");
    write(&root, "src/app.rs", "fn f() { panic!(\"x\"); }\n");

    let report = ian_lint::lint(&root);
    assert!(report.diagnostics.is_empty());
    assert!(report.failures.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn lint_reports_a_walk_error_as_a_failure() {
    let root = temp_root("missing-lint").join("does-not-exist");
    let report = ian_lint::lint(&root);
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].tool, Tool::IanLint);
    assert!(!report.failures[0].output.is_empty());
}
