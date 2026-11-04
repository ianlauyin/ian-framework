use crate::auth::SessionConfig;

use crate::auth::session_store::*;
use crate::test::auth::store_or_skip;

#[tokio::test]
async fn sessions_store_touch_and_remove() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let first = store.create("7").await.unwrap();
    let second = store.create("7").await.unwrap();
    let other = store.create("8").await.unwrap();
    assert_eq!(first.len(), 64);
    assert_ne!(first, second);
    assert_eq!(store.subject(&first).await.unwrap().as_deref(), Some("7"));
    assert!(store.touch(&first).await.unwrap());

    store.remove(&first).await.unwrap();
    assert_eq!(store.subject(&first).await.unwrap(), None);
    assert!(!store.touch(&first).await.unwrap());

    store.remove_subject("7").await.unwrap();
    assert_eq!(store.subject(&second).await.unwrap(), None);
    assert_eq!(store.subject(&other).await.unwrap().as_deref(), Some("8"));
    store.remove(&other).await.unwrap();
}

#[tokio::test]
async fn removing_sessions_leaves_no_subject_index() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let first = store.create("7").await.unwrap();
    let second = store.create("7").await.unwrap();
    let mut conn = store.conn();
    let index = store.subject_key("7");
    let size: usize = redis::cmd("SCARD")
        .arg(&index)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(size, 2);
    store.remove(&first).await.unwrap();
    store.remove_subject("7").await.unwrap();
    assert_eq!(store.subject(&second).await.unwrap(), None);
    let exists: bool = redis::cmd("EXISTS")
        .arg(&index)
        .query_async(&mut conn)
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

    store.remove_subject_except("7", &current).await.unwrap();
    assert_eq!(store.subject(&current).await.unwrap().as_deref(), Some("7"));
    assert_eq!(store.subject(&old).await.unwrap(), None);
    assert_eq!(store.subject(&other).await.unwrap().as_deref(), Some("8"));

    store.remove_subject_except("8", &current).await.unwrap();
    assert_eq!(store.subject(&current).await.unwrap().as_deref(), Some("7"));
    assert_eq!(store.subject(&other).await.unwrap(), None);
    store.remove(&current).await.unwrap();
}

#[tokio::test]
async fn redis_holds_the_token_hash_and_the_deadline() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let token = store.create("7").await.unwrap();
    let mut conn = store.conn();
    let raw: Option<String> = redis::cmd("GET")
        .arg(format!("{}{token}", store.inner.config.prefix))
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(raw, None);
    let value: String = redis::cmd("GET")
        .arg(store.key(&token))
        .query_async(&mut conn)
        .await
        .unwrap();
    let (deadline, subject) = parse_value(&value).unwrap();
    assert_eq!(subject, "7");
    assert!(deadline > now_secs() + 110 && deadline <= now_secs() + 120);
    store.remove(&token).await.unwrap();
}

#[tokio::test]
async fn touch_stops_at_the_deadline() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let mut conn = store.conn();
    let token = "a".repeat(64);
    let key = store.key(&token);
    let near = format!("{}:7", now_secs() + 10);
    redis::cmd("SET")
        .arg(&key)
        .arg(&near)
        .arg("EX")
        .arg(10)
        .query_async::<()>(&mut conn)
        .await
        .unwrap();
    assert!(store.touch(&token).await.unwrap());
    let ttl: i64 = redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(ttl <= 10);

    let past = format!("{}:7", now_secs() - 1);
    redis::cmd("SET")
        .arg(&key)
        .arg(&past)
        .arg("EX")
        .arg(10)
        .query_async::<()>(&mut conn)
        .await
        .unwrap();
    assert_eq!(store.subject(&token).await.unwrap(), None);
    assert!(!store.touch(&token).await.unwrap());
    store.remove(&token).await.unwrap();
}

#[tokio::test]
async fn a_subject_keeps_at_most_the_session_limit() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let mut tokens = Vec::new();
    for _ in 0..MAX_SESSIONS_PER_SUBJECT + 5 {
        tokens.push(store.create("7").await.unwrap());
    }
    let mut conn = store.conn();
    let size: usize = redis::cmd("SCARD")
        .arg(store.subject_key("7"))
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(size, MAX_SESSIONS_PER_SUBJECT);
    let last = tokens.last().unwrap();
    assert_eq!(store.subject(last).await.unwrap().as_deref(), Some("7"));
    store.remove_subject("7").await.unwrap();
}

#[test]
fn value_splits_at_the_first_colon() {
    assert_eq!(parse_value("100:user:7"), Some((100, "user:7")));
    assert_eq!(parse_value("7"), None);
    assert_eq!(parse_value("x:7"), None);
}

#[tokio::test]
async fn debug_shows_only_the_config() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let text = format!("{store:?}");
    assert!(text.contains("cookie_name: \"sid\""));
    assert!(!text.contains("redis://"));
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

    let config = SessionConfig {
        max_age_secs: 0,
        ..SessionConfig::default()
    };
    let err = SessionStore::connect("redis://127.0.0.1:6379", config)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "AUTH_CONFIG");
}

#[test]
fn token_is_64_lowercase_hex_characters() {
    let token = random_token().unwrap();
    assert_eq!(token.len(), 64);
    assert!(token
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    assert_ne!(token, random_token().unwrap());
}

#[test]
fn accepts_only_the_token_shape() {
    assert!(is_token(&"a1".repeat(32)));
    assert!(!is_token(&"a1".repeat(31)));
    assert!(!is_token(&"A1".repeat(32)));
    assert!(!is_token(&"g1".repeat(32)));
    assert!(!is_token(""));
}
