use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::source::with_srcs;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::page_import::check;

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

#[test]
fn a_parent_page_and_the_same_page_pass() {
    let source = "\
import {shared} from \"../shared\";
import {LoginPage} from \"../context\";
import {Form} from \"./Form\";
import {Field} from \"page/account/login/Form/Field\";
import {accountPage} from \"page/account/context\";
import {x} from \"../..\";
export * from \"../../types\";
";
    assert!(diagnostics("web/src/page/account/login/index.tsx", source).is_empty());
}

#[test]
fn a_child_page_and_a_different_folder_pass() {
    let source = "\
import {Detail} from \"./detail\";
import {format} from \"util/format\";
import {Card} from \"component/Card\";
import {PageContext} from \"ian-framework-fe\";
";
    assert!(diagnostics("web/src/page/account/index.tsx", source).is_empty());
    assert!(
        diagnostics(
            "web/src/page/index.tsx",
            "import {Account} from \"./account\";\nimport {Login} from \"page/login\";\n"
        )
        .is_empty()
    );
}

#[test]
fn a_component_folder_of_the_root_page_imports_each_page() {
    let source = "\
import {mainPage} from \"../context\";
import {Account} from \"../account/Main\";
const lazy = import(\"../order/detail/Main\");
";
    assert!(diagnostics("web/src/page/Main/Router.tsx", source).is_empty());
}

#[test]
fn a_component_folder_of_a_parent_page_passes() {
    let source = "\
import {Header} from \"../../Main/Header\";
import {Dialog} from \"page/order/Main/Dialog\";
";
    assert!(diagnostics("web/src/page/order/detail/Main/index.tsx", source).is_empty());
}

#[test]
fn a_component_folder_of_a_sibling_page_fails() {
    assert_eq!(
        diagnostics(
            "web/src/page/order/detail/Main/index.tsx",
            "import {Header} from \"../../list/Main/Header\";\n"
        )
        .len(),
        1
    );
}

#[test]
fn a_file_outside_the_page_folder_passes() {
    assert!(
        diagnostics(
            "web/src/component/Card/index.tsx",
            "import {Login} from \"page/login\";\n"
        )
        .is_empty()
    );
}

#[test]
fn a_sibling_page_fails() {
    let source = "\
import {Signup} from \"../signup/index\";
export {signupPage} from \"page/account/signup/context\";
export * from \"../../order/types\";
import type {Order} from \"page/order/types\";
const lazy = import(\"../signup/Form\");
";
    let found = diagnostics("web/src/page/account/login/index.tsx", source);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [1, 2, 3, 4, 5]);
    assert_eq!(found[0].rule, "page-import");
    assert_eq!(
        found[0].problem,
        "`../signup/index` imports a sibling page."
    );
    assert_eq!(
        found[0].fix,
        "Move the shared code to a parent page folder, to `component/`, or to `util/`."
    );
}

#[test]
fn a_sibling_page_folder_fails_and_a_parent_file_passes() {
    let root = env::temp_dir().join(format!("ian-lint-page-import-{}", process::id()));
    let src = root.join("src");
    fs::create_dir_all(src.join("page/account/login")).unwrap();
    fs::create_dir_all(src.join("page/account/signup")).unwrap();
    let path = src.join("page/account/login/index.tsx");
    let source = "\
import {Signup} from \"../signup\";
import {shared} from \"../shared\";
import {Order} from \"page/order\";
";
    let found = with_srcs(&path, source, &[src.clone()], check);
    fs::remove_dir_all(&root).unwrap();
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [1]);
}

#[test]
fn a_relative_path_out_of_the_src_folder_passes() {
    assert!(
        diagnostics(
            "web/src/page/login/context.ts",
            "import {x} from \"../../../../page/order/types\";\n"
        )
        .is_empty()
    );
}
