use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::cfg_on_mod::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

#[test]
fn a_target_cfg_on_a_mod_line_passes() {
    let source = "\
#[cfg(target_os = \"linux\")]
mod monitor;
#[cfg(all(feature = \"monitor\", target_os = \"linux\"))]
mod disk {
    fn read() {}
}
#[cfg(feature = \"auth\")]
fn auth() {}
";
    assert!(with_source(Path::new("api/src/lib.rs"), source, &srcs(), check).is_empty());
}

#[test]
fn a_target_cfg_on_an_item_fails() {
    let source = "\
#[cfg(target_os = \"linux\")]
fn spawn() {}

#[cfg(not(target_os = \"linux\"))]
use crate::monitor;

struct State {
    #[cfg(target_os = \"linux\")]
    load: u8,
}

fn run() {
    #[cfg(target_os = \"linux\")]
    {
        spawn();
    }
}
";
    let found = with_source(Path::new("api/src/app.rs"), source, &srcs(), check);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [1, 4, 8, 13]);
    assert_eq!(found[0].rule, "cfg-on-mod");
    assert_eq!(
        found[0].problem,
        "`#[cfg(target_os)]` is not on a `mod` line."
    );
    assert_eq!(
        found[0].fix,
        "Put the attribute on the `mod` line of the module."
    );
}

#[test]
fn one_target_block_in_main_passes() {
    let source = "\
#[cfg(target_os = \"linux\")]
mod monitor;

fn main() {
    #[cfg(target_os = \"linux\")]
    {
        monitor::spawn();
    }
}
";
    assert!(with_source(Path::new("api/src/main.rs"), source, &srcs(), check).is_empty());
}

#[test]
fn a_second_target_block_in_main_fails() {
    let source = "\
fn main() {
    #[cfg(target_os = \"linux\")]
    {
        cpu();
    }
    #[cfg(target_os = \"linux\")]
    {
        disk();
    }
}
";
    let found = with_source(Path::new("api/src/main.rs"), source, &srcs(), check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 6);
    assert_eq!(
        found[0].problem,
        "`main.rs` has more than one `#[cfg(target_os)]` block."
    );
    assert_eq!(
        found[0].fix,
        "Start all the code for that target in one block."
    );
}
