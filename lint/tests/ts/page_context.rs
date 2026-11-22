use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::page_context::check;

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

fn problems(found: &[Diagnostic]) -> Vec<&str> {
    found.iter().map(|item| item.problem.as_str()).collect()
}

const SOURCE: &str = "\
import {Logger, PageContext} from \"ian-framework-fe\";
import type {LoginPageStore} from \"./types\";

const initialState = (): LoginPageStore => ({email: \"\"});

export class LoginPage extends PageContext<LoginPageStore> {
    constructor() {
        super(initialState());
    }
}

export const loginPage = new LoginPage();
";

#[test]
fn a_page_context_with_the_class_and_the_initial_state_passes() {
    assert!(diagnostics("web/src/page/login/context.ts", SOURCE).is_empty());
    let constant = SOURCE.replace(
        "const initialState = (): LoginPageStore => ({email: \"\"});",
        "const initialState: LoginPageStore = {email: \"\"};",
    );
    assert!(diagnostics("web/src/page/login/context.ts", &constant).is_empty());
}

#[test]
fn a_file_that_is_not_a_page_context_passes() {
    let source = "export const value = 1;\n";
    for path in [
        "web/src/page/login/types.ts",
        "web/src/page/context.ts",
        "web/src/page/layout/home/context.ts",
        "web/src/module/login/context.ts",
    ] {
        assert!(diagnostics(path, source).is_empty());
    }
}

#[test]
fn a_page_context_with_no_class_and_no_initial_state_fails() {
    let source = "\
import {Logger} from \"ian-framework-fe\";

class LoginPage extends PageContext<object> {}
";
    let found = diagnostics("web/src/page/login/context.ts", source);
    assert_eq!(
        problems(&found),
        [
            "The page context does not import `PageContext` from `ian-framework-fe`.",
            "The page context has no exported class that extends `PageContext`.",
            "The page context has no constant `initialState`.",
        ]
    );
    assert!(found.iter().all(|item| item.rule == "page-context"));
    assert_eq!(
        found[0].fix,
        "Import `PageContext` from `ian-framework-fe`."
    );
    assert_eq!(
        found[1].fix,
        "Write the page context as an exported class that extends `PageContext`."
    );
    assert_eq!(
        found[2].fix,
        "Declare `const initialState` with the store type from `./types`."
    );
}

#[test]
fn an_initial_state_with_a_type_that_is_not_from_types_fails() {
    for line in [
        "const initialState = () => ({email: \"\"});",
        "const initialState = (): Local => ({email: \"\"});",
        "const initialState: object = {};",
    ] {
        let source = SOURCE.replace(
            "const initialState = (): LoginPageStore => ({email: \"\"});",
            line,
        );
        let found = diagnostics("web/src/page/login/context.ts", &source);
        assert_eq!(
            problems(&found),
            ["The type of `initialState` does not come from `./types`."]
        );
        assert_eq!(found[0].line, 4);
        assert_eq!(
            found[0].fix,
            "Give `initialState` the store type from `./types`."
        );
    }
}

#[test]
fn a_page_context_with_no_store_and_no_initial_state_passes() {
    let source = "\
import {PageContext} from \"ian-framework-fe\";

export class LayoutPage extends PageContext<object> {
    constructor() {
        super({});
    }
}
";
    assert!(diagnostics("web/src/page/layout/context.ts", source).is_empty());
}

#[test]
fn a_page_context_with_a_store_and_no_initial_state_fails() {
    let source = "\
import {PageContext} from \"ian-framework-fe\";
import type {LoginPageStore} from \"./types\";

export class LoginPage extends PageContext<LoginPageStore> {}
";
    let found = diagnostics("web/src/page/login/context.ts", source);
    assert_eq!(
        problems(&found),
        ["The page context has no constant `initialState`."]
    );
}
