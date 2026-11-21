use std::fs;
use std::path::Path;

use ian_lint::{Report, Side, check};

use crate::project::{CONFIG, project};

fn report(name: &str, files: &[(&str, &[u8])], side: Side) -> Report {
    let mut all = vec![("lint.config.json", CONFIG.as_bytes())];
    all.extend_from_slice(files);
    let root = project(name, &all);
    let report = check(&root, side);
    fs::remove_dir_all(root).expect("test folder is removed");
    report
}

fn rules(report: &Report) -> Vec<&str> {
    report.diagnostics.iter().map(|item| item.rule).collect()
}

const FILES: &[(&str, &[u8])] = &[
    ("api/src/lib.rs", b"pub static NAME: u32 = 1;\n"),
    ("api/other/lib.rs", b"pub static NAME: u32 = 1;\n"),
    ("web/src/a.ts", b"export default 1;\n"),
    ("web/other/a.ts", b"export default 1;\n"),
];

#[test]
fn a_project_with_no_error_passes() {
    let files: &[(&str, &[u8])] = &[
        ("api/src/lib.rs", b"pub fn run() {}\n"),
        ("web/src/a.ts", b"export const a = 1;\n"),
        ("web/src/a.css", b"a {}\n"),
    ];
    let report = report("run-ok", files, Side::All);
    assert!(report.diagnostics.is_empty());
    assert!(report.failures.is_empty());
}

#[test]
fn each_side_checks_only_its_src_folders() {
    let all = report("run-all", FILES, Side::All);
    assert_eq!(rules(&all), vec!["private-static", "no-default-export"]);
    assert!(
        all.diagnostics[0]
            .path
            .ends_with(Path::new("api/src/lib.rs"))
    );
    assert!(all.diagnostics[1].path.ends_with(Path::new("web/src/a.ts")));
    assert!(all.failures.is_empty());
    let fe = report("run-fe", FILES, Side::Fe);
    assert_eq!(rules(&fe), vec!["no-default-export"]);
    let be = report("run-be", FILES, Side::Be);
    assert_eq!(rules(&be), vec!["private-static"]);
}

#[test]
fn a_file_that_does_not_parse_is_a_failure() {
    let files: &[(&str, &[u8])] = &[
        ("api/src/lib.rs", b"// a comment\nfn ("),
        ("web/src/a.ts", b"// a comment\nexport const = ;\n"),
    ];
    let report = report("run-parse", files, Side::All);
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.failures.len(), 2);
    assert!(
        report.failures[0]
            .output
            .contains("The file does not parse")
    );
}

#[test]
fn a_file_that_the_linter_cannot_read_is_a_failure() {
    let files: &[(&str, &[u8])] = &[("api/src/lib.rs", &[0xff, 0xfe])];
    let report = report("run-read", files, Side::All);
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.failures.len(), 1);
    assert!(report.failures[0].output.contains("lib.rs"));
}

#[test]
fn a_root_with_no_config_is_a_failure() {
    let root = project("run-config", &[]);
    let report = check(&root, Side::All);
    fs::remove_dir_all(root).expect("test folder is removed");
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.failures.len(), 1);
}
