use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::files::{FileSet, Scope, is_frontend_source, is_rust_source, is_style_file, scope};

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

fn write_config(root: &Path) {
    write(
        root,
        "ian-linter.config",
        "{\n  \"ruleset\": \"framework\",\n  \"fe\": { \"path\": [\"src\"] },\n  \"be\": { \"path\": [\"src\"] }\n}\n",
    );
}

#[test]
fn a_source_file_has_one_kind() {
    let fe = [PathBuf::from("web/src")];
    let be = [PathBuf::from("api/src")];
    assert!(is_frontend_source(Path::new("web/src/app.ts"), &fe));
    assert!(is_frontend_source(Path::new("web/src/app.tsx"), &fe));
    assert!(!is_frontend_source(Path::new("web/src/app.css"), &fe));
    assert!(is_style_file(Path::new("web/src/app.css"), &fe));
    assert!(!is_style_file(Path::new("web/src/app.ts"), &fe));
    assert!(!is_style_file(Path::new("other/src/app.css"), &fe));
    assert!(!is_frontend_source(Path::new("web/src/app.d.ts"), &fe));
    assert!(!is_frontend_source(Path::new("web/src/app.rs"), &fe));
    assert!(!is_frontend_source(Path::new("other/src/app.ts"), &fe));
    assert!(is_rust_source(Path::new("api/src/app.rs"), &be));
    assert!(!is_rust_source(Path::new("api/src/app.ts"), &be));
    assert!(!is_rust_source(Path::new("api/tests/app.rs"), &be));
    assert!(!is_rust_source(Path::new("web/src/app.rs"), &be));
}

#[test]
fn scope_uses_the_first_folder_under_the_src_path() {
    let src = Path::new("web/src");
    assert_eq!(
        scope(Path::new("web/src/component/Button/index.css"), src),
        Some(Scope::Component)
    );
    assert_eq!(
        scope(Path::new("web/src/module/account/Panel/index.css"), src),
        Some(Scope::Module)
    );
    assert_eq!(scope(Path::new("web/src/util/x.ts"), src), None);
    assert_eq!(
        scope(Path::new("other/src/component/Button/index.css"), src),
        None
    );
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

#[test]
fn lint_reports_no_diagnostics_when_there_are_no_rules() {
    let root = temp_root("empty-rules");
    write_config(&root);
    write(&root, "src/util/x.ts", "const x = 1;\n");
    write(&root, "src/app.css", ".a { color: red; }\n");
    write(&root, "src/app.rs", "fn f() { panic!(\"x\"); }\n");
    fs::create_dir_all(root.join("src/empty")).expect("folder is created");

    let report = crate::lint(&root);
    assert!(report.diagnostics.is_empty());
    assert!(report.failures.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn lint_reports_a_missing_configuration_file_as_a_failure() {
    let root = temp_root("no-config");
    write(&root, "src/app.ts", "const a = 1;\n");
    let report = crate::lint(&root);
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.failures.len(), 1);
    assert!(report.failures[0].output.contains("ian-linter.config"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn lint_reports_a_walk_error_as_a_failure() {
    let root = temp_root("unreadable");
    write_config(&root);
    let blocked = root.join("blocked");
    fs::create_dir(&blocked).unwrap();
    let mut closed = fs::metadata(&blocked).unwrap().permissions();
    closed.set_mode(0);
    fs::set_permissions(&blocked, closed).unwrap();
    let report = crate::lint(&root);
    let mut open = fs::metadata(&blocked).unwrap().permissions();
    open.set_mode(0o755);
    fs::set_permissions(&blocked, open).unwrap();
    assert!(report.diagnostics.is_empty());
    assert!(
        report
            .failures
            .iter()
            .any(|failure| !failure.output.contains("ian-linter.config"))
    );
    fs::remove_dir_all(root).unwrap();
}
