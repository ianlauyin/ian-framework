use std::env;
use std::io::{IsTerminal as _, stderr, stdout};
use std::path::PathBuf;
use std::process::ExitCode;

use ian_lint::diagnostic::Diagnostic;

const RED: &str = "38;2;220;107;106";
const YELLOW: &str = "38;2;255;215;1";
const GREEN: &str = "32";
const CYAN: &str = "36";
const DIM: &str = "2";

fn main() -> ExitCode {
    let root = env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let out_color = use_color(stdout().is_terminal());
    let err_color = use_color(stderr().is_terminal());

    let report = ian_lint::lint(&root);
    for diagnostic in &report.diagnostics {
        println!("{}", render(diagnostic, out_color));
    }
    for failure in &report.failures {
        let title = paint(
            "ian-lint did not run correctly. This is not a lint error.",
            YELLOW,
            err_color,
        );
        eprintln!(
            "{title} {}\n{}",
            paint("Its output:", DIM, err_color),
            failure.output
        );
    }

    let has_errors = !report.diagnostics.is_empty();
    if has_errors {
        eprintln!(
            "{}",
            paint(
                "lint failed. Correct each error, then run the command again.",
                RED,
                err_color
            )
        );
    }
    if !report.failures.is_empty() {
        ExitCode::from(2)
    } else if has_errors {
        ExitCode::FAILURE
    } else {
        println!("{}", paint("lint okay", GREEN, out_color));
        ExitCode::SUCCESS
    }
}

fn use_color(is_terminal: bool) -> bool {
    let set = |name: &str| env::var(name).is_ok_and(|value| !value.is_empty() && value != "0");
    if env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()) {
        return false;
    }
    set("FORCE_COLOR") || is_terminal
}

fn paint(text: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

fn highlight(text: &str, color: bool) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some((before, after)) = rest.split_once('`') {
        let Some((quoted, tail)) = after.split_once('`') else {
            break;
        };
        out += before;
        out += &paint(&format!("`{quoted}`"), YELLOW, color);
        rest = tail;
    }
    out + rest
}

fn render(diagnostic: &Diagnostic, color: bool) -> String {
    let location = format!(
        "{}:{}:{}",
        diagnostic.path.display(),
        diagnostic.line,
        diagnostic.column
    );
    let rule = format!("ian-lint/{}", diagnostic.rule);
    let text = format!(
        "{} {}: \n{}",
        paint(&location, CYAN, color),
        paint(&rule, RED, color),
        highlight(&diagnostic.problem, color)
    );
    format!(
        "{text}\n    {} {}\n",
        paint("fix:", GREEN, color),
        highlight(&diagnostic.fix, color)
    )
}
