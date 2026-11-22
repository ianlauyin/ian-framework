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
export function Button(props: Props) {
    return props.children;
}

interface Props {
    children: string;
}
";
    let arrow = "export const Button = (props: Props<string>) => props.children;\n";
    let inline = "export function Button(props: {children: string}) {}\n";
    let none = "export function Button() {}\n";
    for source in [function, arrow, inline, none] {
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
