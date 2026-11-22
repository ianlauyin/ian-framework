use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::ajax_name::check;

const PATH: &str = "web/src/util/ajax/api/public/account/register_verify.ts";

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

const SOURCE: &str = "\
import {ajax} from \"ian-framework-fe\";

interface Detail {
    name: string;
}

export interface AccountRegisterVerifyRequest {
    code: string;
}

export interface AccountRegisterVerifyResponse {
    detail: Detail;
}

export function accountRegisterVerifyAjax(request: AccountRegisterVerifyRequest) {
    return ajax(\"/account/register/verify\", \"POST\", request);
}
";

#[test]
fn a_call_with_the_name_of_its_path_passes() {
    assert!(diagnostics(PATH, SOURCE).is_empty());
    let source = "export function accountLogoutAjax() {}\n";
    for path in [
        "web/src/util/ajax/api/protected/account/logout.ts",
        "web/src/util/ajax/account/logout.ts",
    ] {
        assert!(diagnostics(path, source).is_empty());
    }
}

#[test]
fn a_file_that_is_not_an_ajax_file_passes() {
    let source = "export interface PageRequest {}\n";
    for path in [
        "web/src/util/ajax/shared-types.ts",
        "web/src/util/format.ts",
    ] {
        assert!(diagnostics(path, source).is_empty());
    }
}

#[test]
fn a_function_with_a_different_name_fails() {
    let source = "export function registerVerifyAjax() {}\n";
    let found = diagnostics(PATH, source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, "ajax-name");
    assert_eq!(
        found[0].problem,
        "The function `registerVerifyAjax` does not have the name `accountRegisterVerifyAjax`."
    );
    assert_eq!(
        found[0].fix,
        "Name the function from the path of the file, and end the name with `Ajax`."
    );
}

#[test]
fn a_file_with_no_exported_function_fails() {
    let found = diagnostics(PATH, "function accountRegisterVerifyAjax() {}\n");
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].problem,
        "The file has no exported function `accountRegisterVerifyAjax`."
    );
}

#[test]
fn a_type_with_a_different_name_or_place_fails() {
    let source = "\
export interface VerifyRequest {}

const PATH = \"/account/register/verify\";

export function accountRegisterVerifyAjax() {}

export interface AccountRegisterVerifyResponse {}
";
    let found = diagnostics(PATH, source);
    let problems: Vec<_> = found
        .iter()
        .map(|item| (item.line, item.problem.as_str()))
        .collect();
    assert_eq!(
        problems,
        [
            (
                1,
                "The type `VerifyRequest` does not have the name `AccountRegisterVerifyRequest`."
            ),
            (
                1,
                "The type `VerifyRequest` is not immediately before its function."
            ),
            (
                7,
                "The type `AccountRegisterVerifyResponse` is not immediately before its function."
            ),
        ]
    );
    assert_eq!(found[0].fix, "Name the type from the name of the function.");
    assert_eq!(
        found[1].fix,
        "Put the type immediately before the function."
    );
}
