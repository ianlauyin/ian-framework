use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::import_alias::check;

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

#[test]
fn an_alias_path_from_outside_the_folder_passes() {
    let source = "\
import {Card} from \"component/Card\";
import {format} from \"util/format\";
import {i18n} from \"i18n\";
import {en} from \"i18n/en\";
import {loginPage} from \"../context\";
import {Form} from \"./Form\";
import \"./index.css\";
";
    assert!(diagnostics("web/src/page/login/Main/index.tsx", source).is_empty());
}

#[test]
fn a_relative_path_in_the_same_folder_passes() {
    let source = "\
import {Icon} from \"../Icon\";
import {i18n} from \"i18n\";
";
    assert!(diagnostics("web/src/component/Button/index.tsx", source).is_empty());
    assert!(diagnostics("web/src/i18n/index.ts", "import {en} from \"./en\";\n").is_empty());
    assert!(
        diagnostics(
            "web/src/util/date.ts",
            "import {pad} from \"util/format\";\n"
        )
        .is_empty()
    );
}

#[test]
fn a_relative_path_into_an_alias_folder_fails() {
    let source = "\
import {Card} from \"../../../component/Card\";
export {format} from \"../../../util/format\";
const i18n = import(\"../../../i18n\");
";
    let found = diagnostics("web/src/page/login/Main/index.tsx", source);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [1, 2, 3]);
    assert_eq!(found[0].rule, "import-alias");
    assert_eq!(
        found[0].problem,
        "`../../../component/Card` is a relative path into `component/`."
    );
    assert_eq!(found[0].fix, "Import the file with the alias `component`.");
    assert_eq!(
        diagnostics("web/src/App.tsx", "import {i18n} from \"./i18n\";\n").len(),
        1
    );
}

#[test]
fn an_alias_path_in_the_component_folder_fails() {
    let source = "import {Icon} from \"component/Icon\";\n";
    let found = diagnostics("web/src/component/Button/index.tsx", source);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].problem,
        "`component/Icon` is an alias path in `component/`."
    );
    assert_eq!(found[0].fix, "Use a relative path.");
}

#[test]
fn a_path_into_the_module_folder_is_not_checked() {
    let source = "\
import {accountModule} from \"module/account\";
export * from \"../../../module/account\";
";
    assert!(diagnostics("web/src/page/login/Main/index.tsx", source).is_empty());
    assert!(diagnostics("web/src/index.tsx", "import \"./module/account\";\n").is_empty());
}
