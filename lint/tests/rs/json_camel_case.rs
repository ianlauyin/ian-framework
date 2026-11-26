use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::json_camel_case::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

const PATH: &str = "api/src/router/api/account/login.rs";

#[test]
fn camel_case_json_types_pass() {
    let source = "\
#[derive(Deserialize)]
#[serde(rename_all = \"camelCase\")]
struct LoginRequest {}

#[derive(Serialize)]
#[serde(rename_all(serialize = \"camelCase\"))]
struct LoginResponse {}

#[derive(Serialize)]
#[serde(deny_unknown_fields, rename_all = \"camelCase\")]
struct Item {}

async fn login(Json(request): Json<LoginRequest>) -> Result<Json<Vec<Item>>, Exception> {}

async fn other() -> Json<LoginResponse> {}

async fn external(Json(value): Json<serde_json::Value>) {}
";
    assert!(with_source(Path::new(PATH), source, &srcs(), check).is_empty());
}

#[test]
fn a_json_type_without_camel_case_fails() {
    let source = "\
#[derive(Deserialize)]
struct LoginRequest {}

#[derive(Serialize)]
#[serde(rename_all = \"snake_case\")]
struct Item {}

async fn login(Json(request): Json<LoginRequest>) -> Result<Json<Vec<Item>>, Exception> {}
";
    let found = with_source(Path::new(PATH), source, &srcs(), check);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].rule, "json-camel-case");
    assert_eq!(found[0].line, 2);
    assert_eq!(
        found[0].problem,
        "The type `LoginRequest` in the `Json` request of the handler is not camelCase."
    );
    assert_eq!(
        found[0].fix,
        "Add `#[serde(rename_all = \"camelCase\")]` to the type."
    );
    assert_eq!(found[1].line, 6);
    assert_eq!(
        found[1].problem,
        "The type `Item` in the `Json` response of the handler is not camelCase."
    );
}

#[test]
fn only_the_direction_of_the_use_is_checked() {
    let source = "\
#[serde(rename_all(serialize = \"camelCase\"))]
struct Body {}

async fn login(Json(request): Json<Body>) {}
";
    assert_eq!(
        with_source(Path::new(PATH), source, &srcs(), check).len(),
        1
    );
}

#[test]
fn a_file_outside_a_router_folder_passes() {
    let source = "struct Body {}\n\nasync fn find(Json(body): Json<Body>) {}\n";
    for path in ["api/src/db/account.rs", "api/src/tests/router/login.rs"] {
        assert!(with_source(Path::new(path), source, &srcs(), check).is_empty());
    }
}
