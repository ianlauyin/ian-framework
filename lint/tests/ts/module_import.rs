use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::module_import::check;

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

#[test]
fn the_index_in_the_src_folder_passes() {
    let source = "\
import \"module/account\";
import {cartModule} from \"./module/cart\";
";
    assert!(diagnostics("web/src/index.tsx", source).is_empty());
}

#[test]
fn a_file_in_the_same_module_passes() {
    let source = "\
import type {AccountStore} from \"./types\";
import {format} from \"module/account/format\";
";
    assert!(diagnostics("web/src/module/account/index.ts", source).is_empty());
}

#[test]
fn a_type_import_passes() {
    let source = "\
import type {AccountStore} from \"module/account/types\";
export type {AccountStore} from \"module/account/types\";
export type * from \"module/account/types\";
";
    assert!(diagnostics("web/src/page/login/context.ts", source).is_empty());
}

#[test]
fn an_import_of_a_different_folder_passes() {
    let source = "\
import {format} from \"util/format\";
import {Card} from \"component/Card\";
import {Module} from \"ian-framework-fe\";
import {modulePath} from \"module-path\";
import {loginPage} from \"../context\";
";
    assert!(diagnostics("web/src/page/login/Main/index.tsx", source).is_empty());
}

#[test]
fn an_import_of_a_module_in_a_different_file_fails() {
    let source = "\
import {accountModule} from \"module/account\";
export {cartModule} from \"module/cart\";
export * from \"../../../module/cart\";
import \"../../../module\";
const lazy = import(\"module/account\");
";
    let found = diagnostics("web/src/page/login/Main/index.tsx", source);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [1, 2, 3, 4, 5]);
    assert_eq!(found[0].rule, "module-import");
    assert_eq!(
        found[0].problem,
        "`module/account` imports a module outside `index.tsx`."
    );
    assert_eq!(
        found[0].fix,
        "Use the module through `this.module` in a page context. Only `index.tsx` in the src folder imports a module."
    );
}

#[test]
fn a_different_module_and_a_nested_index_fail() {
    assert_eq!(
        diagnostics(
            "web/src/module/account/index.ts",
            "import {cartModule} from \"../cart\";\n"
        )
        .len(),
        1
    );
    assert_eq!(
        diagnostics(
            "web/src/page/index.tsx",
            "import {cartModule} from \"module/cart\";\n"
        )
        .len(),
        1
    );
    assert_eq!(
        diagnostics(
            "web/src/App.tsx",
            "import {cartModule} from \"./module/cart\";\n"
        )
        .len(),
        1
    );
}

#[test]
fn a_relative_path_out_of_the_src_folder_passes() {
    assert!(
        diagnostics(
            "web/src/page/login/context.ts",
            "import {x} from \"../../../../module/account\";\n"
        )
        .is_empty()
    );
}
