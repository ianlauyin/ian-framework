use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::module_shape::check;

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

const SOURCE: &str = "\
import {Module} from \"ian-framework-fe\";
import type {UserSettingStore} from \"./types\";

const initialStore: UserSettingStore = {name: null};

export class UserSettingModule extends Module<UserSettingStore> {
    constructor() {
        super(initialStore);
    }
}
";

const INDEX_SOURCE: &str = "\
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
fn a_module_with_the_class_passes() {
    assert!(diagnostics("web/src/module/user-setting/index.ts", SOURCE).is_empty());
}

#[test]
fn a_file_that_is_not_the_index_of_a_module_passes() {
    let source = "export interface UserSettingStore {}\n";
    for path in [
        "web/src/module/user-setting/types.ts",
        "web/src/util/index.ts",
    ] {
        assert!(diagnostics(path, source).is_empty());
    }
}

#[test]
fn a_type_in_the_index_of_a_module_fails() {
    let source = SOURCE.replace(
        "const initialStore",
        "interface Local {}\nexport type Name = string;\nconst initialStore",
    );
    let found = diagnostics("web/src/module/user-setting/index.ts", &source);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 4);
    assert_eq!(found[0].rule, "module-shape");
    assert_eq!(
        found[0].problem,
        "The type `Local` is in the `index.ts` of a module."
    );
    assert_eq!(found[0].fix, "Move the type to `types.ts`.");
    assert_eq!(found[1].line, 5);
}

#[test]
fn a_module_with_a_different_class_fails() {
    let source = "\
import {Module} from \"ian-framework-fe\";
import type {Store} from \"./types\";

const initialStore: Store = {};

export class Settings extends Module<Store> {}
";
    let found = diagnostics("web/src/module/user-setting/index.ts", source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 1);
    assert_eq!(
        found[0].problem,
        "The module has no exported class `UserSettingModule` that extends `Module`."
    );
    assert_eq!(
        found[0].fix,
        "Write the module as `export class UserSettingModule extends Module<UserSettingStore>`."
    );
}

#[test]
fn a_module_with_no_module_import_or_no_initial_store_fails() {
    let source = SOURCE
        .replace("import {Module}", "import {Module as Base}")
        .replace("const initialStore: UserSettingStore = {name: null};\n", "");
    let found = diagnostics("web/src/module/user-setting/index.ts", &source);
    let problems: Vec<_> = found.iter().map(|item| item.problem.as_str()).collect();
    assert_eq!(
        problems,
        [
            "The module does not import `Module` from `ian-framework-fe`.",
            "The module has no constant `initialStore`.",
        ]
    );
    assert_eq!(found[0].fix, "Import `Module` from `ian-framework-fe`.");
    assert_eq!(
        found[1].fix,
        "Declare `const initialStore` with the store type from `./types`."
    );
}

#[test]
fn an_initial_store_with_a_type_that_is_not_from_types_fails() {
    for line in [
        "const initialStore = {name: null};",
        "const initialStore: Local = {name: null};",
        "const initialStore: {name: null} = {name: null};",
    ] {
        let source = SOURCE.replace("const initialStore: UserSettingStore = {name: null};", line);
        let found = diagnostics("web/src/module/user-setting/index.ts", &source);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 4);
        assert_eq!(
            found[0].problem,
            "The type of `initialStore` does not come from `./types`."
        );
        assert_eq!(
            found[0].fix,
            "Give `initialStore` the store type from `./types`."
        );
    }
}

#[test]
fn an_initial_store_that_is_a_function_passes() {
    let source = SOURCE.replace(
        "const initialStore: UserSettingStore = {name: null};",
        "const initialStore = (): UserSettingStore => ({name: null});",
    );
    assert!(diagnostics("web/src/module/user-setting/index.ts", &source).is_empty());
}

#[test]
fn other_code_in_the_index_of_a_module_fails() {
    for line in [
        "const helper = 1;",
        "const noRates = (): Record<string, number> => ({});",
        "export const other = 1;",
        "function helper() {}",
    ] {
        let source = format!("{SOURCE}\n{line}\n");
        let found = diagnostics("web/src/module/user-setting/index.ts", &source);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].problem,
            "The `index.ts` of a module has code that is not `initialStore` or the module class."
        );
        assert_eq!(
            found[0].fix,
            "Keep only `initialStore` and the module class in the file."
        );
    }
}

#[test]
fn a_module_index_with_the_registration_shape_passes() {
    assert!(diagnostics("web/src/module/index.ts", INDEX_SOURCE).is_empty());
}

#[test]
fn a_module_index_with_an_alias_module_import_passes() {
    let source = INDEX_SOURCE.replace("./user-setting", "module/user-setting");
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
        let source =
            INDEX_SOURCE.replace("import {UserSettingModule} from \"./user-setting\";", line);
        let found = diagnostics("web/src/module/index.ts", &source);
        assert!(found.iter().any(|item| item.problem
            == format!(
                "The module index import `{}` is not `registerModule` or a module class.",
                import_source(line)
            )));
    }
}

#[test]
fn a_module_index_without_the_register_import_or_module_map_fails() {
    let source = INDEX_SOURCE
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
fn other_code_in_a_module_index_fails() {
    for line in [
        "const helper = 1;",
        "export const other = 1;",
        "interface Local {}",
    ] {
        let source = format!("{INDEX_SOURCE}\n{line}\n");
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
