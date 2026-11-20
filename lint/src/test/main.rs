use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::{env, fs, process};

fn temp_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("ian-lint-main-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    fs::create_dir_all(&root).expect("test folder is created");
    root
}

fn ian_lint() -> PathBuf {
    let exe = env::current_exe().expect("test executable");
    exe.ancestors()
        .map(|dir| dir.join("ian-lint"))
        .find(|path| path.is_file())
        .expect("ian-lint is built")
}

fn run(root: &Path, vars: &[(&str, &str)]) -> Output {
    command(vars).arg(root).output().expect("ian-lint runs")
}

fn run_in(dir: &Path, args: &[&OsStr]) -> Output {
    command(&[])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("ian-lint runs")
}

fn command(vars: &[(&str, &str)]) -> Command {
    let mut command = Command::new(ian_lint());
    command.env_remove("NO_COLOR").env_remove("FORCE_COLOR");
    for (name, value) in vars {
        command.env(name, value);
    }
    command
}

fn has_escape(output: &Output) -> bool {
    output.stdout.contains(&0x1b) || output.stderr.contains(&0x1b)
}

fn write_config(root: &Path) {
    fs::write(
        root.join(crate::config::FILE_NAME),
        "{\n  \"ruleset\": \"framework\",\n  \"fe\": { \"paths\": [\"src\"] },\n  \"be\": { \"paths\": [] }\n}\n",
    )
    .expect("config is written");
}

#[test]
fn a_root_with_no_diagnostics_exits_with_code_0() {
    let root = temp_root("clean");
    write_config(&root);
    fs::create_dir_all(root.join("src")).expect("folder is created");
    fs::write(root.join("src/app.ts"), "const a = 1;\n").expect("file is written");
    fs::write(root.join("src/index.ts"), "export * from \"./app\";\n").expect("file is written");

    let output = run(&root, &[]);
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"lint okay\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn a_root_without_a_configuration_file_exits_with_code_2() {
    let root = temp_root("no-config");
    fs::create_dir_all(root.join("src")).expect("folder is created");
    fs::write(root.join("src/app.ts"), "const a = 1;\n").expect("file is written");

    let output = run(&root, &[]);
    fs::remove_dir_all(&root).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(stderr.contains("ian-lint did not run correctly. This is not a lint error."));
    assert!(stderr.contains(crate::config::FILE_NAME));
}

#[test]
fn a_root_that_the_walk_cannot_read_exits_with_code_2_and_prints_the_failure_on_stderr() {
    let root = temp_root("unreadable");
    write_config(&root);
    let blocked = root.join("blocked");
    fs::create_dir(&blocked).expect("folder is created");
    let mut closed = fs::metadata(&blocked)
        .expect("folder metadata")
        .permissions();
    closed.set_mode(0);
    fs::set_permissions(&blocked, closed).expect("folder is unreadable");

    let output = run(&root, &[]);
    let mut open = fs::metadata(&blocked)
        .expect("folder metadata")
        .permissions();
    open.set_mode(0o755);
    fs::set_permissions(&blocked, open).expect("folder is readable");
    fs::remove_dir_all(&root).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(stderr.contains("ian-lint did not run correctly. This is not a lint error."));
    assert!(stderr.contains("blocked"));
}

#[test]
fn no_color_turns_the_color_off_even_with_force_color() {
    let base = temp_root("no-color");
    let root = base.join("does-not-exist");

    let output = run(&root, &[("NO_COLOR", "1"), ("FORCE_COLOR", "1")]);
    fs::remove_dir_all(&base).unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!has_escape(&output));
}

#[test]
fn an_empty_no_color_does_not_turn_the_color_off() {
    let base = temp_root("empty-no-color");
    let root = base.join("does-not-exist");

    let output = run(&root, &[("NO_COLOR", ""), ("FORCE_COLOR", "1")]);
    fs::remove_dir_all(&base).unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(has_escape(&output));
}

#[test]
fn force_color_0_keeps_the_color_off() {
    let base = temp_root("force-color-0");
    let root = base.join("does-not-exist");

    let output = run(&root, &[("FORCE_COLOR", "0")]);
    fs::remove_dir_all(&base).unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!has_escape(&output));
}

#[test]
fn output_that_is_not_a_terminal_has_no_color() {
    let base = temp_root("not-terminal");
    let root = base.join("does-not-exist");

    let output = run(&root, &[]);
    fs::remove_dir_all(&base).unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!has_escape(&output));
}

#[test]
fn force_color_turns_the_color_on_when_the_output_is_not_a_terminal() {
    let base = temp_root("force-color");
    let root = base.join("does-not-exist");

    let output = run(&root, &[("FORCE_COLOR", "1")]);
    fs::remove_dir_all(&base).unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(has_escape(&output));
}

fn write_both(root: &Path, fe: &str, rs: &str) {
    fs::write(
        root.join(crate::config::FILE_NAME),
        "{\n  \"ruleset\": \"framework\",\n  \"fe\": { \"paths\": [\"src\"] },\n  \"be\": { \"paths\": [\"src\"] }\n}\n",
    )
    .expect("config is written");
    fs::create_dir_all(root.join("src")).expect("folder is created");
    fs::write(root.join("src/app.ts"), fe).expect("file is written");
    fs::write(root.join("src/index.ts"), "export * from \"./app\";\n").expect("file is written");
    fs::write(root.join("src/app.rs"), rs).expect("file is written");
}

#[test]
fn fe_reports_a_frontend_comment_and_skips_rust() {
    let root = temp_root("arg-fe");
    write_both(&root, "// note\nconst a = 1;\n", "// note\nfn f() {}\n");

    let output = run_in(&root, &[root.as_os_str(), OsStr::new("fe")]);
    fs::remove_dir_all(&root).unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout.contains("src/app.ts"));
    assert!(!stdout.contains("src/app.rs"));
}

#[test]
fn fe_with_no_root_checks_the_current_folder() {
    let root = temp_root("arg-fe-cwd");
    write_both(&root, "const a = 1;\n", "// note\nfn f() {}\n");

    let output = run_in(&root, &[OsStr::new("fe")]);
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"lint okay\n");
}

#[test]
fn rs_before_the_root_reports_a_rust_comment_and_skips_the_frontend() {
    let root = temp_root("arg-rs");
    write_both(&root, "// note\nconst a = 1;\n", "// note\nfn f() {}\n");

    let output = run_in(&root, &[OsStr::new("rs"), root.as_os_str()]);
    fs::remove_dir_all(&root).unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout.contains("src/app.rs"));
    assert!(!stdout.contains("src/app.ts"));
}

#[test]
fn rs_reports_no_diagnostics_when_only_the_frontend_has_a_comment() {
    let root = temp_root("arg-rs-clean");
    write_both(&root, "// note\nconst a = 1;\n", "fn f() {}\n");

    let output = run_in(&root, &[OsStr::new("rs")]);
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"lint okay\n");
}

#[test]
fn two_sides_stop_the_run() {
    let root = temp_root("arg-two-sides");
    write_config(&root);

    let output = run_in(&root, &[OsStr::new("fe"), OsStr::new("rs")]);
    fs::remove_dir_all(&root).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("The side is set two times."));
}

#[test]
fn format_fe_rewrites_the_frontend_and_skips_rust() {
    let root = temp_root("format-fe");
    write_both(
        &root,
        "export const name = \"a\";\nconst value = 1;\n",
        "fn helper() {}\n",
    );

    let output = run_in(&root, &[OsStr::new("format"), OsStr::new("fe")]);
    let frontend = fs::read_to_string(root.join("src/app.ts")).unwrap();
    let rust = fs::read_to_string(root.join("src/app.rs")).unwrap();
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"lint okay\n");
    assert!(frontend.find("const value").unwrap() < frontend.find("export const").unwrap());
    assert_eq!(rust, "fn helper() {}\n");
}

#[test]
fn two_formats_stop_the_run() {
    let root = temp_root("arg-two-formats");
    write_config(&root);

    let output = run_in(&root, &[OsStr::new("format"), OsStr::new("format")]);
    fs::remove_dir_all(&root).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("The format is set two times."));
}

#[test]
fn two_roots_stop_the_run() {
    let root = temp_root("arg-two-roots");
    write_config(&root);

    let output = run_in(&root, &[OsStr::new("one"), OsStr::new("two")]);
    fs::remove_dir_all(&root).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("The root is set two times."));
}
