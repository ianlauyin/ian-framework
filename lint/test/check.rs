use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::{env, fs, process};

fn temp_root(name: &str) -> std::path::PathBuf {
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
        crate::config::FILE_NAME,
        "{\n  \"ruleset\": \"framework\",\n  \"fe\": { \"paths\": [\"src\"] },\n  \"be\": { \"paths\": [\"src\"] }\n}\n",
    );
}

fn write_both(root: &Path, fe: &str, rs: &str) {
    write_config(root);
    write(root, "src/app.ts", fe);
    write(root, "src/index.ts", "export * from \"./app\";\n");
    write(root, "src/app.rs", rs);
}

fn has_path(report: &crate::Report, suffix: &str) -> bool {
    report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.path.ends_with(suffix))
}

#[test]
fn check_reports_no_diagnostics_for_source_that_passes() {
    let root = temp_root("empty-rules");
    write_config(&root);
    write(&root, "src/util/x.ts", "const x = 1;\n");
    write(&root, "src/util/index.ts", "export { x } from \"./x\";\n");
    write(
        &root,
        "src/index.ts",
        "export * from \"./util\";\nexport * from \"./empty\";\n",
    );
    write(&root, "src/empty/index.ts", "");
    write(&root, "src/app.css", ".a { color: red; }\n");
    write(&root, "src/app.rs", "fn f() { panic!(\"x\"); }\n");

    let report = crate::check::check(&root, crate::config::Side::All);
    assert!(report.diagnostics.is_empty());
    assert!(report.failures.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_reports_a_missing_configuration_file_as_a_failure() {
    let root = temp_root("no-config");
    write(&root, "src/app.ts", "const a = 1;\n");
    let report = crate::check::check(&root, crate::config::Side::All);
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.failures.len(), 1);
    assert!(report.failures[0].output.contains(crate::config::FILE_NAME));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_reports_a_walk_error_as_a_failure() {
    let root = temp_root("unreadable");
    write_config(&root);
    let blocked = root.join("blocked");
    fs::create_dir(&blocked).unwrap();
    let mut closed = fs::metadata(&blocked).unwrap().permissions();
    closed.set_mode(0);
    fs::set_permissions(&blocked, closed).unwrap();
    let report = crate::check::check(&root, crate::config::Side::All);
    let mut open = fs::metadata(&blocked).unwrap().permissions();
    open.set_mode(0o755);
    fs::set_permissions(&blocked, open).unwrap();
    assert!(report.diagnostics.is_empty());
    assert!(
        report
            .failures
            .iter()
            .any(|failure| !failure.output.contains(crate::config::FILE_NAME))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_with_no_side_reports_a_frontend_comment_and_a_rust_comment() {
    let root = temp_root("side-all");
    write_both(&root, "// note\nconst a = 1;\n", "// note\nfn f() {}\n");

    let report = crate::check::check(&root, crate::config::Side::All);
    assert!(report.failures.is_empty());
    assert!(has_path(&report, "src/app.ts"));
    assert!(has_path(&report, "src/app.rs"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_fe_reports_a_frontend_comment_and_skips_rust() {
    let root = temp_root("side-fe");
    write_both(&root, "// note\nconst a = 1;\n", "// note\nfn f() {}\n");

    let report = crate::check::check(&root, crate::config::Side::Fe);
    assert!(report.failures.is_empty());
    assert!(has_path(&report, "src/app.ts"));
    assert!(!has_path(&report, "src/app.rs"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_fe_reports_no_diagnostics_when_only_rust_has_a_comment() {
    let root = temp_root("side-fe-clean");
    write_both(&root, "const a = 1;\n", "// note\nfn f() {}\n");

    let report = crate::check::check(&root, crate::config::Side::Fe);
    assert!(report.failures.is_empty());
    assert!(report.diagnostics.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_rs_reports_a_rust_comment_and_skips_the_frontend() {
    let root = temp_root("side-rs");
    write_both(&root, "// note\nconst a = 1;\n", "// note\nfn f() {}\n");

    let report = crate::check::check(&root, crate::config::Side::Rs);
    assert!(report.failures.is_empty());
    assert!(has_path(&report, "src/app.rs"));
    assert!(!has_path(&report, "src/app.ts"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_rs_reports_no_diagnostics_when_only_the_frontend_has_a_comment() {
    let root = temp_root("side-rs-clean");
    write_both(&root, "// note\nconst a = 1;\n", "fn f() {}\n");

    let report = crate::check::check(&root, crate::config::Side::Rs);
    assert!(report.failures.is_empty());
    assert!(report.diagnostics.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_reports_a_file_that_does_not_parse_as_a_failure() {
    let root = temp_root("parse-error");
    write_both(&root, "const a = ;\n", "fn f( {}\n");

    let report = crate::check::check(&root, crate::config::Side::All);
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.failures.len(), 2);
    assert!(report.failures[0].output.contains("src/app.rs:1:"));
    assert!(report.failures[1].output.contains("src/app.ts:1:"));
    fs::remove_dir_all(root).unwrap();
}
