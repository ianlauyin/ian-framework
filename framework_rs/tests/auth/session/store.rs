use ian_framework_rs::auth::session::config::SessionConfig;
use ian_framework_rs::auth::session::store::*;

use crate::auth::{LIMIT, connection, store_or_skip};

async fn subject_of(store: &SessionStore, token: &str) -> Option<String> {
    store
        .load(token)
        .await
        .unwrap()
        .map(|session| session.subject)
}

async fn set_size(store: &SessionStore, subject: &str) -> usize {
    redis::cmd("SCARD")
        .arg(store.subject_key(subject))
        .query_async(&mut store.conn())
        .await
        .unwrap()
}

async fn put_raw(store: &SessionStore, token: &str, value: &str, expiry: u64) {
    redis::cmd("SET")
        .arg(store.key(token))
        .arg(value)
        .arg("EX")
        .arg(expiry)
        .query_async::<()>(&mut store.conn())
        .await
        .unwrap();
}

#[tokio::test]
async fn sessions_load_and_remove() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let first = store.create("7").await.unwrap();
    let second = store.create("7").await.unwrap();
    let other = store.create("8").await.unwrap();
    assert_eq!(first.token.len(), 64);
    assert_ne!(first.token, second.token);
    assert_eq!(first.subject, "7");

    let loaded = store.load(&first.token).await.unwrap().unwrap();
    assert_eq!(loaded, first);

    store.remove(&first.token).await.unwrap();
    assert_eq!(store.load(&first.token).await.unwrap(), None);

    store.remove_subject("7").await.unwrap();
    assert_eq!(subject_of(&store, &second.token).await, None);
    assert_eq!(subject_of(&store, &other.token).await.as_deref(), Some("8"));
    store.remove_subject("8").await.unwrap();
}

#[tokio::test]
async fn remove_subject_leaves_no_set() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let first = store.create("7").await.unwrap();
    store.create("7").await.unwrap();
    assert_eq!(set_size(&store, "7").await, 2);
    store.remove(&first.token).await.unwrap();
    assert_eq!(set_size(&store, "7").await, 1);
    store.remove_subject("7").await.unwrap();
    let exists: bool = redis::cmd("EXISTS")
        .arg(store.subject_key("7"))
        .query_async(&mut store.conn())
        .await
        .unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn remove_subject_except_keeps_one_session() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let current = store.create("7").await.unwrap();
    let old = store.create("7").await.unwrap();
    let other = store.create("8").await.unwrap();

    store
        .remove_subject_except("7", &current.token)
        .await
        .unwrap();
    assert_eq!(
        subject_of(&store, &current.token).await.as_deref(),
        Some("7")
    );
    assert_eq!(subject_of(&store, &old.token).await, None);
    assert_eq!(subject_of(&store, &other.token).await.as_deref(), Some("8"));
    assert_eq!(set_size(&store, "7").await, 1);

    store
        .remove_subject_except("8", &current.token)
        .await
        .unwrap();
    assert_eq!(
        subject_of(&store, &current.token).await.as_deref(),
        Some("7")
    );
    assert_eq!(subject_of(&store, &other.token).await, None);
    store.remove_subject("7").await.unwrap();
}

#[tokio::test]
async fn redis_holds_the_token_hash_and_the_subject() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = store.create("7").await.unwrap();
    let key = store.key(&session.token);
    let prefix = &key[..key.len() - 64];
    let mut conn = store.conn();
    let raw: Option<String> = redis::cmd("GET")
        .arg(format!("{prefix}{}", session.token))
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(raw, None);
    let value: String = redis::cmd("GET")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&value).unwrap();
    assert_eq!(stored["subject"], "7");
    let left = stored["expires_at"].as_u64().unwrap() - now_secs();
    assert!(left > 3590 && left <= 3600);
    store.remove_subject("7").await.unwrap();
}

#[tokio::test]
async fn load_slides_the_expiry_to_the_ttl() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let token = "a".repeat(64);
    put_raw(&store, &token, "7", 5).await;
    assert_eq!(subject_of(&store, &token).await.as_deref(), Some("7"));
    let ttl: i64 = redis::cmd("TTL")
        .arg(store.key(&token))
        .query_async(&mut store.conn())
        .await
        .unwrap();
    assert!(ttl > 5 && ttl <= 60);
    store.remove(&token).await.unwrap();
}

async fn set_ttl(store: &SessionStore, subject: &str) -> i64 {
    redis::cmd("TTL")
        .arg(store.subject_key(subject))
        .query_async(&mut store.conn())
        .await
        .unwrap()
}

#[tokio::test]
async fn the_subject_set_gets_the_ttl_at_each_create_and_load() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = store.create("7").await.unwrap();
    let ttl = set_ttl(&store, "7").await;
    assert!(ttl > 5 && ttl <= 60);

    for start in [Start::Load(session.token.clone()), Start::Create] {
        redis::cmd("EXPIRE")
            .arg(store.subject_key("7"))
            .arg(5)
            .query_async::<()>(&mut store.conn())
            .await
            .unwrap();
        match start {
            Start::Load(token) => assert!(store.load(&token).await.unwrap().is_some()),
            Start::Create => drop(store.create("7").await.unwrap()),
        }
        let ttl = set_ttl(&store, "7").await;
        assert!(ttl > 5 && ttl <= 60);
    }
    store.remove_subject("7").await.unwrap();
}

#[tokio::test]
async fn load_of_a_missing_session_makes_no_key() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let token = "d".repeat(64);
    assert_eq!(store.load(&token).await.unwrap(), None);
    let exists: bool = redis::cmd("EXISTS")
        .arg(store.key(&token))
        .query_async(&mut store.conn())
        .await
        .unwrap();
    assert!(!exists);
}

enum Start {
    Load(String),
    Create,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn key_ttl(store: &SessionStore, token: &str) -> i64 {
    redis::cmd("TTL")
        .arg(store.key(token))
        .query_async(&mut store.conn())
        .await
        .unwrap()
}

fn stored(subject: &str, expires_at: u64) -> String {
    serde_json::json!({ "subject": subject, "expires_at": expires_at }).to_string()
}

#[tokio::test]
async fn a_session_ends_at_the_absolute_limit_also_when_it_is_in_use() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let near = "e".repeat(64);
    put_raw(&store, &near, &stored("7", now_secs() + 10), 30).await;
    assert_eq!(subject_of(&store, &near).await.as_deref(), Some("7"));
    let ttl = key_ttl(&store, &near).await;
    assert!(ttl > 0 && ttl <= 10);

    let past = "f".repeat(64);
    put_raw(&store, &past, &stored("7", now_secs() - 1), 30).await;
    assert_eq!(subject_of(&store, &past).await, None);
    assert_eq!(key_ttl(&store, &past).await, -2);
    store.remove(&near).await.unwrap();
}

#[tokio::test]
async fn a_session_without_an_end_time_gets_the_absolute_limit_at_its_first_load() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let token = "9".repeat(64);
    put_raw(&store, &token, "7", 30).await;
    assert_eq!(subject_of(&store, &token).await.as_deref(), Some("7"));
    let value: String = redis::cmd("GET")
        .arg(store.key(&token))
        .query_async(&mut store.conn())
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&value).unwrap();
    assert_eq!(value["subject"], "7");
    let first = value["expires_at"].as_u64().unwrap();
    assert!(first > now_secs() + 3590 && first <= now_secs() + 3600);
    let ttl = key_ttl(&store, &token).await;
    assert!(ttl > 30 && ttl <= 60);

    assert_eq!(subject_of(&store, &token).await.as_deref(), Some("7"));
    let again: String = redis::cmd("GET")
        .arg(store.key(&token))
        .query_async(&mut store.conn())
        .await
        .unwrap();
    let again: serde_json::Value = serde_json::from_str(&again).unwrap();
    assert_eq!(again["expires_at"].as_u64().unwrap(), first);
    store.remove(&token).await.unwrap();
    assert_eq!(key_ttl(&store, &token).await, -2);
}

#[tokio::test]
async fn a_subject_keeps_at_most_the_session_limit() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let mut sessions = Vec::new();
    for _ in 0..LIMIT + 5 {
        sessions.push(store.create("7").await.unwrap());
    }
    assert_eq!(set_size(&store, "7").await, LIMIT);
    let newest = sessions.last().unwrap();
    assert_eq!(
        subject_of(&store, &newest.token).await.as_deref(),
        Some("7")
    );
    store.remove_subject("7").await.unwrap();
}

#[tokio::test]
async fn over_the_limit_expired_keys_go_first_then_the_least_recently_used() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let mut conn = store.conn();
    let gone = store.key(&"b".repeat(64));
    let idle = "c".repeat(64);
    put_raw(&store, &idle, "7", 30).await;
    redis::cmd("SADD")
        .arg(store.subject_key("7"))
        .arg(&gone)
        .arg(store.key(&idle))
        .query_async::<()>(&mut conn)
        .await
        .unwrap();
    for _ in 0..LIMIT - 1 {
        store.create("7").await.unwrap();
    }
    assert_eq!(set_size(&store, "7").await, LIMIT);
    let is_member: bool = redis::cmd("SISMEMBER")
        .arg(store.subject_key("7"))
        .arg(&gone)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(!is_member);
    let exists: bool = redis::cmd("EXISTS")
        .arg(store.key(&idle))
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(exists);

    store.create("7").await.unwrap();
    assert_eq!(set_size(&store, "7").await, LIMIT);
    assert_eq!(subject_of(&store, &idle).await, None);
    store.remove_subject("7").await.unwrap();
}

#[tokio::test]
async fn debug_shows_only_the_config() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let text = format!("{store:?}");
    assert!(text.contains("cookie_name: \"sid\""));
    assert!(!text.contains("redis://"));
    let session = store.create("7").await.unwrap();
    let text = format!("{session:?}");
    assert!(text.contains("subject: \"7\""));
    assert!(!text.contains(&session.token));
    store.remove_subject("7").await.unwrap();
}

#[tokio::test]
async fn bad_config_is_refused_before_connecting() {
    let Some(conn) = connection().await else {
        return;
    };
    let config = SessionConfig {
        ttl_secs: 0,
        ..SessionConfig::default()
    };
    let err = SessionStore::new(conn, config).unwrap_err();
    assert_eq!(err.code(), "SESSION_CONFIG_INVALID");
}
