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

fn run(root: &Path, vars: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ian-lint"));
    command
        .arg(root)
        .env_remove("NO_COLOR")
        .env_remove("FORCE_COLOR");
    for (name, value) in vars {
        command.env(name, value);
    }
    command.output().expect("ian-lint runs")
}

fn has_escape(output: &Output) -> bool {
    output.stdout.contains(&0x1b) || output.stderr.contains(&0x1b)
}

fn write_config(root: &Path) {
    fs::write(
        root.join("ian-linter.config"),
        "{\n  \"ruleset\": \"framework\",\n  \"fe\": { \"path\": [\"src\"] },\n  \"be\": { \"path\": [] }\n}\n",
    )
    .expect("config is written");
}

#[test]
fn a_root_with_no_diagnostics_exits_with_code_0() {
    let root = temp_root("clean");
    write_config(&root);
    fs::create_dir_all(root.join("src")).expect("folder is created");
    fs::write(root.join("src/app.ts"), "const a = 1;\n").expect("file is written");

    let output = run(&root, &[]);
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
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
    assert!(stderr.contains("ian-linter.config"));
}

#[test]
fn a_root_that_the_walk_cannot_read_exits_with_code_2_and_prints_the_failure_on_stderr() {
    let base = temp_root("missing");
    let root = base.join("does-not-exist");

    let output = run(&root, &[]);
    fs::remove_dir_all(&base).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(stderr.contains("ian-lint did not run correctly. This is not a lint error."));
    assert!(stderr.contains("does-not-exist"));
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
