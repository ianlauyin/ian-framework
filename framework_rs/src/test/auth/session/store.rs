use crate::auth::session::config::SessionConfig;
use crate::auth::session::store::*;
use crate::test::auth::{store_or_skip, LIMIT};

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
    assert_eq!(set_size(&store, "7").await, 2);
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
    assert_eq!(value, "7");
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

#[test]
fn tls_url_is_accepted() {
    assert!(redis::Client::open("rediss://default:secret@example.upstash.io:6379").is_ok());
}

#[tokio::test]
async fn bad_config_is_refused_before_connecting() {
    let err = SessionStore::connect("not a redis url", SessionConfig::default())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "AUTH_CONFIG");
    assert!(!err.to_string().contains("not a redis url"));

    let config = SessionConfig {
        ttl_secs: 0,
        ..SessionConfig::default()
    };
    let err = SessionStore::connect("redis://127.0.0.1:6379", config)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "AUTH_CONFIG");
}
