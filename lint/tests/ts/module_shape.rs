use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::module_shape::check;

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

const SOURCE: &str = "\
import {Module, registerModule} from \"ian-framework-fe\";
import type {UserSettingStore} from \"./types\";

const initialStore: UserSettingStore = {name: null};

export class UserSettingModule extends Module<UserSettingStore> {
    constructor() {
        super(initialStore);
    }
}

registerModule(\"userSetting\", new UserSettingModule());
";

#[test]
fn a_module_with_the_class_and_the_register_passes() {
    assert!(diagnostics("web/src/module/user-setting/index.ts", SOURCE).is_empty());
}

#[test]
fn a_file_that_is_not_the_index_of_a_module_passes() {
    let source = "export interface UserSettingStore {}\n";
    for path in [
        "web/src/module/user-setting/types.ts",
        "web/src/util/index.ts",
        "web/src/module/index.ts",
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
fn a_module_with_a_different_class_or_no_register_fails() {
    let source = "\
import {Module} from \"ian-framework-fe\";
import type {Store} from \"./types\";

const initialStore: Store = {};

export class Settings extends Module<Store> {}

export const settings = new Settings();
";
    let found = diagnostics("web/src/module/user-setting/index.ts", source);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 1);
    assert_eq!(
        found[0].problem,
        "The module has no exported class `UserSettingModule` that extends `Module`."
    );
    assert_eq!(
        found[0].fix,
        "Write the module as `export class UserSettingModule extends Module<UserSettingStore>`."
    );
    assert_eq!(found[1].line, 8);
    assert_eq!(
        found[1].problem,
        "The last statement does not register the module `userSetting`."
    );
    assert_eq!(
        found[1].fix,
        "End the file with `registerModule(\"userSetting\", new UserSettingModule());`."
    );
}

#[test]
fn a_module_with_no_module_import_or_no_initial_store_fails() {
    let source = SOURCE
        .replace("import {Module,", "import {Module as Base,")
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
fn a_register_with_a_different_name_or_an_exported_instance_fails() {
    for line in [
        "registerModule(\"settings\", new UserSettingModule());",
        "registerModule(\"userSetting\", new Other());",
        "register(\"userSetting\", new UserSettingModule());",
        "export const userSettingModule = new UserSettingModule();",
    ] {
        let source = SOURCE.replace(
            "registerModule(\"userSetting\", new UserSettingModule());",
            line,
        );
        let found = diagnostics("web/src/module/user-setting/index.ts", &source);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 12);
        assert_eq!(
            found[0].problem,
            "The last statement does not register the module `userSetting`."
        );
    }
}
