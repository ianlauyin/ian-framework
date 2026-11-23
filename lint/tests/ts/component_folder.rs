use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::component_folder::check;

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

#[test]
fn a_component_with_the_name_of_its_folder_passes() {
    let function = "\
export interface Props {
    children: string;
}

export function Button(props: Props) {
    return props.children;
}
";
    let arrow_interface = "export interface Props {\n    children: string;\n}\n\nexport const Button = (props: Props) => props.children;\n";
    let imported =
        "import type {Props} from \"./types\";\nexport function Button(props: Props) {}\n";
    let generic = "export const Button = (props: Props<string>) => props.children;\n";
    let none = "export function Button() {}\n";
    for source in [function, arrow_interface, imported, generic, none] {
        assert!(diagnostics("web/src/component/Button/index.tsx", source).is_empty());
    }
    assert!(diagnostics("web/src/component/Dialog/Button/index.tsx", function).is_empty());
}

#[test]
fn a_file_that_is_not_the_index_of_a_component_passes() {
    let source = "export function Other(props: OtherProps) {}\n";
    for path in [
        "web/src/component/Button/part.tsx",
        "web/src/page/login/Main/index.tsx",
    ] {
        assert!(diagnostics(path, source).is_empty());
    }
}

#[test]
fn a_component_with_a_different_name_fails() {
    let source = "export function Other() {}\nfunction Button() {}\n";
    let found = diagnostics("web/src/component/Button/index.tsx", source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "component-folder");
    assert_eq!(found[0].problem, "The file has no named export `Button`.");
    assert_eq!(
        found[0].fix,
        "Export the component with the name of its folder."
    );
}

#[test]
fn a_props_type_with_a_different_name_fails() {
    let source = "export function Button(props: ButtonProps) {}\n";
    let found = diagnostics("web/src/component/Button/index.tsx", source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].column, 31);
    assert_eq!(
        found[0].problem,
        "The props type `ButtonProps` does not have the name `Props`."
    );
    assert_eq!(found[0].fix, "Name the props type `Props`.");
}

#[test]
fn a_props_type_that_is_not_exported_fails() {
    let source =
        "interface Props {\n    children: string;\n}\n\nexport function Button(props: Props) {}\n";
    let found = diagnostics("web/src/component/Button/index.tsx", source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "component-folder");
    assert_eq!(found[0].problem, "The props type `Props` is not exported.");
    assert_eq!(found[0].fix, "Export the props type.");
}

#[test]
fn a_props_type_after_the_component_fails() {
    let source = "export function Button(props: Props) {}\n\nexport interface Props {\n    children: string;\n}\n";
    let found = diagnostics("web/src/component/Button/index.tsx", source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(
        found[0].problem,
        "The props type `Props` is after the component."
    );
    assert_eq!(found[0].fix, "Declare the props type before the component.");
}

#[test]
fn a_private_props_alias_after_the_component_gives_three_errors() {
    let source =
        "export const Button = (props: Props) => null;\ntype Props = {children: string};\n";
    let found = diagnostics("web/src/component/Button/index.tsx", source);
    let problems: Vec<_> = found.iter().map(|item| item.problem.as_str()).collect();
    assert_eq!(
        problems,
        vec![
            "The props type `Props` is not an interface.",
            "The props type `Props` is not exported.",
            "The props type `Props` is after the component.",
        ]
    );
}

#[test]
fn a_props_type_alias_fails() {
    let source = "export type Props = {children: string};\nexport const Button = (props: Props) => props.children;\n";
    let found = diagnostics("web/src/component/Button/index.tsx", source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 1);
    assert_eq!(
        found[0].problem,
        "The props type `Props` is not an interface."
    );
    assert_eq!(found[0].fix, "Declare the props type as an interface.");
}

#[test]
fn a_props_type_that_is_not_a_type_name_fails() {
    let inline = "export function Button(props: {children: string}) {}\n";
    let qualified = "export function Button(props: Types.Props) {}\n";
    let union = "export function Button(props: Props | null) {}\n";
    for source in [inline, qualified, union] {
        let found = diagnostics("web/src/component/Button/index.tsx", source);
        assert_eq!(found.len(), 1, "{source}");
        assert_eq!(found[0].column, 31);
        assert_eq!(
            found[0].problem,
            "The props type does not have the name `Props`."
        );
        assert_eq!(
            found[0].fix,
            "Name the props type `Props`, and declare it as an interface."
        );
    }
}
