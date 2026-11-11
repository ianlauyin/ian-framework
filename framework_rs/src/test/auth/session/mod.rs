mod config;
mod store;
mod token;

use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;
use axum::http::Request;

use crate::auth::session::*;

fn parts(change: impl FnOnce(&mut Parts)) -> Parts {
    let (mut parts, _) = Request::new(()).into_parts();
    change(&mut parts);
    parts
}

fn session() -> Session {
    Session {
        token: "a".repeat(64),
        subject: "7".to_string(),
        expires_in_secs: 60,
    }
}

async fn required(parts: &mut Parts) -> Result<Session, &'static str> {
    <Session as FromRequestParts<()>>::from_request_parts(parts, &())
        .await
        .map_err(|err| err.code())
}

async fn optional(parts: &mut Parts) -> Result<Option<Session>, &'static str> {
    <Session as OptionalFromRequestParts<()>>::from_request_parts(parts, &())
        .await
        .map_err(|err| err.code())
}

#[tokio::test]
async fn a_loaded_session_is_given_to_the_handler() {
    let mut loaded = parts(|parts| {
        parts.extensions.insert(session());
    });
    assert_eq!(required(&mut loaded).await, Ok(session()));
    assert_eq!(optional(&mut loaded).await, Ok(Some(session())));
}

#[tokio::test]
async fn no_session_is_unauthorized_only_when_required() {
    let mut empty = parts(|_| {});
    assert_eq!(required(&mut empty).await, Err("UNAUTHORIZED"));
    assert_eq!(optional(&mut empty).await, Ok(None));
}

#[tokio::test]
async fn a_failed_load_is_a_store_error() {
    let mut failed = parts(|parts| {
        parts.extensions.insert(LoadFailed);
    });
    assert_eq!(required(&mut failed).await, Err("AUTH_STORE"));
    assert_eq!(optional(&mut failed).await, Err("AUTH_STORE"));
}
