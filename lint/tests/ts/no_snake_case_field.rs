use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::no_snake_case_field::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

#[test]
fn a_camel_case_field_and_a_snake_case_value_pass() {
    let source = "\
export interface Client {
    birthDate: string | null;
    _private: number;
    id_: string;
}
type Pair = {groupId: string};
enum Kind {CriticalIllness = \"critical_illness\"}
const record = {critical_illness: 1};
const birth_date = 1;
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_snake_case_field_fails() {
    let source = "\
export interface Client {
    birth_date: string | null;
    \"group_id\": string;
}
type Pair = {updated_at: number};
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].rule, "no-snake-case-field");
    assert_eq!(found[0].problem, "The field `birth_date` is snake_case.");
    assert_eq!(found[0].fix, "Rename the field in camelCase.");
    assert_eq!(found[1].problem, "The field `group_id` is snake_case.");
    assert_eq!(found[2].line, 5);
}
