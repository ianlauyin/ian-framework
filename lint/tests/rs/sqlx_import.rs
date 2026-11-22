use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::sqlx_import::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

#[test]
fn sqlx_in_the_db_folder_passes() {
    let source = "\
use sqlx::PgPool;

#[derive(sqlx::FromRow)]
struct Row;

fn load() {
    sqlx::query(\"select 1\");
}
";
    for path in ["api/src/db/user.rs", "api/src/db/user/row.rs"] {
        assert!(with_source(Path::new(path), source, &srcs(), check).is_empty());
    }
}

#[test]
fn sqlx_in_a_test_file_passes() {
    let source = "use sqlx::PgPool;\n";
    for path in ["api/src/tests/app.rs", "api/tests/app.rs"] {
        assert!(with_source(Path::new(path), source, &srcs(), check).is_empty());
    }
}

#[test]
fn a_file_with_no_sqlx_passes() {
    let source = "\
use crate::db::user::load;
use other::sqlx::Name;

fn sample(sqlx: u8) -> u8 {
    let _ = \"sqlx::query\";
    sqlx
}
";
    assert!(with_source(Path::new("api/src/app.rs"), source, &srcs(), check).is_empty());
}

#[test]
fn sqlx_outside_the_db_folder_fails() {
    let source = "\
use sqlx::PgPool;
use sqlx as sql;
use {sqlx::Row, other::Name};

#[derive(Clone, sqlx::FromRow)]
struct Item;

fn load(pool: &sqlx::PgPool) {
    ::sqlx::query(\"select 1\");
}
";
    for path in ["api/src/app.rs", "api/src/db.rs", "api/src/user/db/row.rs"] {
        let found = with_source(Path::new(path), source, &srcs(), check);
        let lines: Vec<_> = found.iter().map(|item| item.line).collect();
        assert_eq!(lines, [1, 2, 3, 5, 8, 9]);
        assert_eq!(found[0].rule, "sqlx-import");
        assert_eq!(found[0].problem, "`sqlx` is used outside the `db` folder.");
        assert_eq!(found[0].fix, "Move the code to the `db` folder.");
        assert!(found.iter().all(|item| item.problem == found[0].problem));
        assert!(found.iter().all(|item| item.fix == found[0].fix));
    }
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/app.rs"), "fn (", &srcs(), check).is_empty());
}

#[test]
fn sqlx_in_a_macro_call_fails() {
    let source = "\
fn load() {
    let _ = vec![sqlx::query(\"select 1\")];
    sqlx::query!(\"select 1\");
}
";
    let found = with_source(Path::new("api/src/app.rs"), source, &srcs(), check);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [2, 3]);
}
