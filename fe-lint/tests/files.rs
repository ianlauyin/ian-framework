#![allow(clippy::tests_outside_test_module, reason = "An integration test file is a test module.")]

use std::path::{Path, PathBuf};
use std::{env, fs, process};

use fe_lint::diagnostic::{Diagnostic, Tool, suppress};
use fe_lint::files::FileSet;

/// A new empty folder for one test.
fn temp_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("fe-lint-{name}-{}", process::id()));
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

fn diagnostic(line: u32, rule: &'static str) -> Diagnostic {
    Diagnostic::new(Path::new("a.css"), line, 1, rule, "problem", "fix")
}

#[test]
fn ignore_comment_stops_its_rule_on_the_same_line_and_the_next_line() {
    let source = "/* code-style-lint-ignore global-prefix: reason */\n.a {}\n.b {}\n.c {} /* code-style-lint-ignore global-prefix */\n";
    let diagnostics = vec![
        diagnostic(1, "global-prefix"),
        diagnostic(2, "global-prefix"),
        diagnostic(2, "class-name-case"),
        diagnostic(3, "global-prefix"),
        diagnostic(4, "global-prefix"),
    ];
    let left: Vec<_> = suppress(source, diagnostics).into_iter().map(|d| (d.line, d.rule)).collect();
    assert_eq!(left, [(2, "class-name-case"), (3, "global-prefix")]);
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
    assert_eq!(files.module_name(Path::new("src/module/account/billing/Form/index.css")), Some("billing"));
    assert_eq!(files.module_name(Path::new("src/module/account/Panel/index.css")), Some("account"));
    assert_eq!(files.module_name(Path::new("src/component/Button/index.css")), None);
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
    assert!(paths.iter().all(|path| !path.components().any(|part| part.as_os_str() == "node_modules")));
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

    let report = fe_lint::lint(&root);
    assert!(report.diagnostics.is_empty());
    assert!(report.failures.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn lint_reports_a_walk_error_as_a_failure() {
    let root = temp_root("missing-lint").join("does-not-exist");
    let report = fe_lint::lint(&root);
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].tool, Tool::CodeStyle);
    assert!(!report.failures[0].output.is_empty());
}
