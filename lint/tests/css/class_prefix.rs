use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::css::class_prefix::check;
use ian_lint::rule::diagnostic::Diagnostic;

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

fn problems(found: &[Diagnostic]) -> Vec<(u32, &str)> {
    found
        .iter()
        .map(|item| (item.line, item.problem.as_str()))
        .collect()
}

#[test]
fn a_component_with_the_shared_prefix_passes() {
    let source = "\
.g-icon-button {
    &.g-icon-button-plain {}
    > .g-icon > svg {}
    &[data-on=\"a.b\"] {}

    @media (max-width: 430px) {
        > .g-icon-button-label {}
    }
}

@keyframes g-icon-button-spin {
    to {}
}
";
    assert!(diagnostics("web/src/component/IconButton/index.css", source).is_empty());
    let part = ".g-icon-button-badge {\n    > .g-icon {}\n}\n";
    assert!(diagnostics("web/src/component/IconButton/Badge/index.css", part).is_empty());
}

#[test]
fn a_component_with_a_different_class_fails() {
    let source = "\
.icon-button {}

.g-icon-button {
    &.plain {}
}
";
    let found = diagnostics("web/src/component/IconButton/index.css", source);
    assert_eq!(
        problems(&found),
        [
            (
                1,
                "The root selector `.icon-button` is not `.g-icon-button`."
            ),
            (4, "The class `plain` does not start with `g-`."),
        ]
    );
    assert_eq!(found[0].rule, "css-class-prefix");
    assert_eq!(
        found[0].fix,
        "Give the outermost tag of the component the class `g-icon-button`."
    );
    assert_eq!(found[1].fix, "Start the class with `g-`.");
    let part = diagnostics(
        "web/src/component/IconButton/Badge/index.css",
        ".g-icon-button {}\n",
    );
    assert_eq!(
        problems(&part),
        [(
            1,
            "The root selector `.g-icon-button` does not start with `.g-icon-button-`."
        )]
    );
}

#[test]
fn a_page_with_the_page_prefix_passes() {
    let page = "\
#login-page {
    > .g-card {}
    > .login-page-main > .login-page-title {}
}
";
    assert!(diagnostics("web/src/page/login/Main/index.css", page).is_empty());
    let part = ".login-page-form {\n    &.login-page-form-wide {}\n}\n";
    assert!(diagnostics("web/src/page/login/Main/Form/index.css", part).is_empty());
    let layout = "#layout-page {}\n";
    assert!(diagnostics("web/src/page/user-layout/Main/index.css", layout).is_empty());
    let child = "#home-page {\n    > .home-page-main {}\n}\n";
    assert!(diagnostics("web/src/page/user-layout/home/index.css", child).is_empty());
}

#[test]
fn a_page_with_a_different_root_selector_fails() {
    let class = diagnostics("web/src/page/login/Main/index.css", ".login-page {}\n");
    assert_eq!(
        problems(&class),
        [(1, "The root selector `.login-page` is a class.")]
    );
    assert_eq!(class[0].fix, "Use the id `#login-page`.");
    let other = diagnostics(
        "web/src/page/login/Main/index.css",
        "#register-page {}\n\n.form {}\n",
    );
    assert_eq!(
        problems(&other),
        [
            (
                1,
                "The root selector `#register-page` does not name the page."
            ),
            (3, "The root selector `.form` does not name the page."),
        ]
    );
    assert_eq!(
        other[0].fix,
        "Use `#login-page` for the outermost tag of the page, or a class that starts with `login-page-`."
    );
}

#[test]
fn a_page_class_with_a_different_prefix_fails() {
    let source = "\
#login-page {
    > .form {}
    > .register-page-form {}
}
";
    let found = diagnostics("web/src/page/login/Main/index.css", source);
    assert_eq!(
        problems(&found),
        [
            (2, "The class `form` does not start with `login-page-`."),
            (
                3,
                "The class `register-page-form` does not start with `login-page-`."
            ),
        ]
    );
    assert_eq!(found[0].fix, "Start the class with `login-page-`.");
}

#[test]
fn a_file_outside_the_component_and_page_folders_passes() {
    for path in ["web/src/index.css", "web/src/style/tokens.css"] {
        assert!(diagnostics(path, ".form {}\n").is_empty());
    }
}
