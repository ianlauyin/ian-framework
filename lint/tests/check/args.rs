use std::ffi::OsString;
use std::path::PathBuf;

use ian_lint::Side;
use ian_lint::args::{Command, USAGE, parse};

fn run(args: &[&str]) -> Result<Command, String> {
    parse(args.iter().map(OsString::from))
}

fn check(root: &str, side: Side) -> Result<Command, String> {
    Ok(Command::Check {
        root: PathBuf::from(root),
        side,
    })
}

#[test]
fn no_argument_checks_both_sides_of_the_current_folder() {
    assert_eq!(run(&[]), check(".", Side::All));
}

#[test]
fn a_side_and_a_root_in_either_order_pass() {
    assert_eq!(run(&["fe"]), check(".", Side::Fe));
    assert_eq!(run(&["app"]), check("app", Side::All));
    assert_eq!(run(&["be", "app"]), check("app", Side::Be));
    assert_eq!(run(&["app", "be"]), check("app", Side::Be));
}

#[test]
fn a_help_argument_gives_the_usage() {
    assert_eq!(run(&["--help"]), Ok(Command::Help));
    assert_eq!(run(&["app", "-h"]), Ok(Command::Help));
}

#[test]
fn two_sides_or_two_roots_fail() {
    assert_eq!(
        run(&["fe", "be"]),
        Err("The side is set two times.".to_owned())
    );
    assert_eq!(
        run(&["app", "other"]),
        Err("The root is set two times.".to_owned())
    );
}

#[test]
fn an_argument_that_starts_with_a_dash_fails() {
    assert_eq!(
        run(&["--fix"]),
        Err(format!("`--fix` is not an argument.\n{USAGE}"))
    );
}
