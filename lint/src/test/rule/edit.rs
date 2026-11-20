use std::path::PathBuf;
use std::{env, fs, process};

use crate::rule::edit::{Edit, apply, write};

fn edit(start: u32, end: u32, text: &str) -> Edit {
    Edit {
        path: PathBuf::new(),
        start,
        end,
        text: text.to_owned(),
    }
}

#[test]
fn an_insert_replaces_an_empty_range() {
    let item = edit(1, 1, "x");
    assert_eq!(apply("ab", &[&item]).unwrap(), "axb");
}

#[test]
fn a_delete_removes_the_range() {
    let item = edit(1, 3, "");
    assert_eq!(apply("abcd", &[&item]).unwrap(), "ad");
}

#[test]
fn two_edits_keep_the_later_offset() {
    let first = edit(0, 1, "AA");
    let second = edit(3, 4, "B");
    assert_eq!(apply("abcdef", &[&first, &second]).unwrap(), "AAbcBef");
}

#[test]
fn overlapping_edits_are_rejected() {
    let first = edit(0, 2, "x");
    let second = edit(1, 3, "y");
    assert!(apply("abcd", &[&first, &second]).is_err());
}

#[test]
fn an_edit_outside_the_file_is_rejected() {
    let item = edit(0, 8, "x");
    assert!(apply("abcd", &[&item]).is_err());
}

fn temp_file(name: &str, text: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("ian-lint-edit-{name}-{}", process::id()));
    fs::write(&path, text).expect("file is written");
    path
}

#[test]
fn overlapping_edits_leave_the_file_unchanged() {
    let path = temp_file("overlap", "abcd");
    let first = Edit {
        path: path.clone(),
        start: 0,
        end: 2,
        text: "x".to_owned(),
    };
    let second = Edit {
        path: path.clone(),
        start: 1,
        end: 3,
        text: "y".to_owned(),
    };
    let failures = write(&[first, second]);
    let text = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert_eq!(text, "abcd");
    assert_eq!(failures.len(), 1);
    assert!(failures[0].output.contains("the edits overlap."));
}

#[test]
fn an_edit_outside_the_file_leaves_the_file_unchanged() {
    let path = temp_file("range", "abcd");
    let item = Edit {
        path: path.clone(),
        start: 0,
        end: 8,
        text: "x".to_owned(),
    };
    let failures = write(&[item]);
    let text = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert_eq!(text, "abcd");
    assert_eq!(failures.len(), 1);
    assert!(failures[0].output.contains("an edit is outside the file."));
}

#[test]
fn a_valid_edit_writes_the_file() {
    let path = temp_file("write", "abcd");
    let item = Edit {
        path: path.clone(),
        start: 1,
        end: 3,
        text: "X".to_owned(),
    };
    let failures = write(&[item]);
    let text = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(failures.is_empty());
    assert_eq!(text, "aXd");
}
