use std::path::{Path, PathBuf};

use crate::rule::rs::file::with_source;
use crate::rule::rs::mutex_lock::check;

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
