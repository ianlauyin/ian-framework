use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::module_registration::check;

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

const SOURCE: &str = "\
import {registerModule} from \"ian-framework-fe\";
import {UserSettingModule} from \"./user-setting\";

declare module \"ian-framework-fe\" {
    interface ModuleMap {
        userSetting: UserSettingModule;
    }
}

registerModule(\"userSetting\", new UserSettingModule());
";

#[test]
fn a_module_index_with_the_registration_shape_passes() {
    assert!(diagnostics("web/src/module/index.ts", SOURCE).is_empty());
}

#[test]
fn a_file_that_is_not_the_module_index_passes() {
    for path in [
        "web/src/module/user-setting/index.ts",
        "web/src/util/index.ts",
    ] {
        assert!(diagnostics(path, "const other = 1;\n").is_empty());
    }
}

#[test]
fn a_module_index_with_an_alias_module_import_passes() {
    let source = SOURCE.replace("./user-setting", "module/user-setting");
    assert!(diagnostics("web/src/module/index.ts", &source).is_empty());
}

#[test]
fn a_module_index_with_a_bad_import_fails() {
    for line in [
        "import {UserSettingModule} from \"./types\";",
        "import type {UserSettingModule} from \"./user-setting\";",
        "import {UserSettingModule as Module} from \"./user-setting\";",
        "import {Module} from \"ian-framework-fe\";",
    ] {
        let source = SOURCE.replace("import {UserSettingModule} from \"./user-setting\";", line);
        let found = diagnostics("web/src/module/index.ts", &source);
        assert!(found.iter().any(|item| item.problem
            == format!(
                "The module index import `{}` is not `registerModule` or a module class.",
                import_source(line)
            )));
        assert_eq!(found[0].rule, "module-registration");
    }
}

#[test]
fn a_module_index_without_the_register_import_or_module_map_fails() {
    let source = SOURCE
        .replace("import {registerModule} from \"ian-framework-fe\";\n", "")
        .replace(
            "declare module \"ian-framework-fe\" {\n    interface ModuleMap {\n        userSetting: UserSettingModule;\n    }\n}\n\n",
            "",
        );
    let found = diagnostics("web/src/module/index.ts", &source);
    let problems: Vec<_> = found.iter().map(|item| item.problem.as_str()).collect();
    assert_eq!(
        problems,
        [
            "The module index does not import `registerModule` from `ian-framework-fe`.",
            "The module index does not declare `ModuleMap` in `ian-framework-fe`.",
        ]
    );
    assert_eq!(
        found[0].fix,
        "Import `registerModule` from `ian-framework-fe`."
    );
    assert_eq!(
        found[1].fix,
        "Declare module `ian-framework-fe` with interface `ModuleMap`."
    );
}

#[test]
fn a_module_index_without_a_registration_fails() {
    let source = SOURCE.replace(
        "registerModule(\"userSetting\", new UserSettingModule());\n",
        "",
    );
    let found = diagnostics("web/src/module/index.ts", &source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
    assert_eq!(
        found[0].problem,
        "The module index does not register `UserSettingModule`."
    );
    assert_eq!(
        found[0].fix,
        "Call `registerModule(\"userSetting\", new UserSettingModule());`."
    );
}

#[test]
fn a_module_index_with_a_wrong_registration_fails() {
    for line in [
        "registerModule(\"settings\", new UserSettingModule());",
        "registerModule(\"userSetting\", new OtherModule());",
        "registerModule(\"userSetting\", new UserSettingModule(1));",
        "registerModule(\"userSetting\");",
    ] {
        let source = SOURCE.replace(
            "registerModule(\"userSetting\", new UserSettingModule());",
            line,
        );
        let found = diagnostics("web/src/module/index.ts", &source);
        assert!(
            found
                .iter()
                .any(|item| item.problem
                    == "The module index does not register `UserSettingModule`.")
        );
    }
}

#[test]
fn other_code_in_a_module_index_fails() {
    for line in [
        "const helper = 1;",
        "export const other = 1;",
        "interface Local {}",
    ] {
        let source = format!("{SOURCE}\n{line}\n");
        let found = diagnostics("web/src/module/index.ts", &source);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].problem,
            "`module/index.ts` has code outside the module registration shape."
        );
        assert_eq!(
            found[0].fix,
            "Keep only module imports, the `ModuleMap` declaration, and the `registerModule` calls."
        );
    }
}

fn import_source(line: &str) -> &str {
    line.rsplit_once("from \"")
        .and_then(|(_, rest)| rest.strip_suffix("\";"))
        .unwrap_or("")
}
