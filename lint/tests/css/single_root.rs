use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::css::single_root::check;
use ian_lint::rule::diagnostic::Diagnostic;

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

const SOURCE: &str = "\
.g-button {
    .g-button-spinner {}
}

.g-button-plain,
.g-button-wide {}
";

#[test]
fn an_index_css_with_one_root_selector_passes() {
    let source = "\
.g-button {
    .g-button-spinner {}
    &:hover, &:focus {}
}

@keyframes g-button-spin {
    to {}
}
";
    assert!(diagnostics("web/src/component/Button/index.css", source).is_empty());
}

#[test]
fn a_file_that_the_rule_does_not_check_passes() {
    for path in ["web/src/index.css", "web/src/style/tokens.css"] {
        assert!(diagnostics(path, SOURCE).is_empty());
    }
}

#[test]
fn an_index_css_with_more_than_one_root_selector_fails() {
    let found = diagnostics("web/src/component/Button/index.css", SOURCE);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [5, 6]);
    assert_eq!(found[0].rule, "css-single-root");
    assert_eq!(
        found[0].problem,
        "The file has more than one root selector."
    );
    assert_eq!(
        found[0].fix,
        "Keep one root selector. Nest each other selector in its block."
    );
}
