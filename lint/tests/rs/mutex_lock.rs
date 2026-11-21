use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::mutex_lock::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

#[test]
fn mutex_lock_and_an_await_pass() {
    let source = "\
fn sample(lock: Lock) {
    mutex::lock(&lock);
    let _ = lock.lock().await;
    let _ = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
}
";
    assert!(with_source(Path::new("api/src/work.rs"), source, &srcs(), check).is_empty());
    let allowed = "\
fn sample(lock: Lock) {
    let _ = lock.lock().unwrap();
    let _ = lock.lock().unwrap_or(1);
}
";
    assert!(with_source(Path::new("api/src/mutex.rs"), allowed, &srcs(), check).is_empty());
}

#[test]
fn lock_unwrap_in_a_test_file_passes() {
    let source = "\
fn sample(lock: Lock) {
    let _ = lock.lock().unwrap();
}
";
    assert!(with_source(Path::new("api/src/tests/work.rs"), source, &srcs(), check).is_empty());
}

#[test]
fn lock_unwrap_outside_mutex_rs_fails() {
    let source = "\
fn sample(lock: Lock) {
    let _ = lock.lock().unwrap();
    let _ = lock.lock().unwrap_or(1);
}
";
    let found = with_source(Path::new("api/src/work.rs"), source, &srcs(), check);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].rule, "mutex-lock");
    assert_eq!(found[0].problem, "`lock().unwrap` locks a standard mutex.");
    assert_eq!(found[0].fix, "Lock it with `mutex::lock`.");
    assert_eq!(found[1].line, 3);
    assert_eq!(
        found[1].problem,
        "`lock().unwrap_or` locks a standard mutex."
    );
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/work.rs"), "fn (", &srcs(), check).is_empty());
}

#[test]
fn lock_expect_fails() {
    let source = "\
fn sample(lock: Lock) {
    let _ = lock.lock().expect(\"the lock is free\");
}
";
    let found = with_source(Path::new("api/src/work.rs"), source, &srcs(), check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].problem, "`lock().expect` locks a standard mutex.");
}

#[test]
fn lock_unwrap_in_a_macro_call_fails() {
    let source = "\
fn sample(lock: Lock) {
    assert!(lock.lock().unwrap().is_empty());
    let _ = vec![lock.lock().unwrap(); 2];
    println!(\"{}\", format!(\"{:?}\", lock.lock().unwrap()));
}
";
    let lines: Vec<_> = with_source(Path::new("api/src/work.rs"), source, &srcs(), check)
        .into_iter()
        .map(|item| item.line)
        .collect();
    assert_eq!(lines, vec![2, 3, 4]);
}

#[test]
fn a_macro_call_that_is_not_rust_code_has_no_diagnostic() {
    let source = "\
fn sample(lock: Lock) {
    custom!(lock => lock().unwrap() ;; @);
}
";
    assert!(with_source(Path::new("api/src/work.rs"), source, &srcs(), check).is_empty());
}
