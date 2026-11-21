use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::ts::test_path::check;

#[test]
fn a_source_file_passes() {
    let source = "export const a = 1;\n";
    assert!(with_source(Path::new("app/src/a.ts"), source, check).is_empty());
    assert!(with_source(Path::new("app/src/test.tsx"), source, check).is_empty());
}

#[test]
fn a_test_file_in_the_source_folder_fails() {
    let source = "export const a = 1;\n";
    let found = with_source(Path::new("app/src/http/a.test.ts"), source, check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, Path::new("app/src/http/a.test.ts"));
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "test-path");
    assert_eq!(found[0].problem, "A test file is in the source folder.");
    assert_eq!(found[0].fix, "Move the test to the `tests` folder.");
    assert_eq!(
        with_source(Path::new("app/src/http/b.test.tsx"), source, check).len(),
        1
    );
}
