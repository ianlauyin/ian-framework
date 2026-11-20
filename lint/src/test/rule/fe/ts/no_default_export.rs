use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::rule::fe::ts::file::with_source;
use crate::rule::fe::ts::no_default_export::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

#[test]
fn a_named_export_passes() {
    let source = "\
export const value = 1;
export { value as other };
export { read } from \"./read\";
export { default as value } from \"./read\";
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_default_export_fails() {
    let source = "\
export const value = 1;
export default value;
export { value as default };
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].rule, "no-default-export");
    assert_eq!(found[0].problem, "The file has a default export.");
    assert_eq!(found[0].fix, "Use a named export.");
    assert_eq!(found[1].line, 3);
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}
