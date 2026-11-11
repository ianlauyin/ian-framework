use crate::auth::session::config::SessionConfig;
use crate::auth::session::store::*;
use crate::auth::session::Session;
use crate::test::auth::store_or_skip;
use crate::time::now_secs;

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

async fn put_raw(store: &SessionStore, token: &str, value: String, expiry: u64) {
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
    assert_eq!(first.expires_in_secs, 60);

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
async fn a_subject_with_a_colon_loads_whole() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = store.create("user:7:admin").await.unwrap();
    assert_eq!(
        subject_of(&store, &session.token).await.as_deref(),
        Some("user:7:admin")
    );
    store.remove_subject("user:7:admin").await.unwrap();
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
async fn redis_holds_the_token_hash_and_the_deadline() {
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
    let (deadline, subject) = value.split_once(':').unwrap();
    let deadline: u64 = deadline.parse().unwrap();
    assert_eq!(subject, "7");
    assert!(deadline > now_secs() + 110 && deadline <= now_secs() + 120);
    store.remove_subject("7").await.unwrap();
}

#[tokio::test]
async fn load_slides_the_expiry_up_to_the_deadline() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let token = "a".repeat(64);
    put_raw(&store, &token, format!("{}:7", now_secs() + 300), 5).await;
    let session = store.load(&token).await.unwrap().unwrap();
    assert_eq!(session.expires_in_secs, 60);

    put_raw(&store, &token, format!("{}:7", now_secs() + 10), 5).await;
    let session = store.load(&token).await.unwrap().unwrap();
    assert!(session.expires_in_secs <= 10 && session.expires_in_secs >= 9);
    let ttl: i64 = redis::cmd("TTL")
        .arg(store.key(&token))
        .query_async(&mut store.conn())
        .await
        .unwrap();
    assert!(ttl <= 10);

    put_raw(&store, &token, format!("{}:7", now_secs() - 1), 10).await;
    assert_eq!(store.load(&token).await.unwrap(), None);

    put_raw(&store, &token, "not-a-value".to_string(), 10).await;
    assert_eq!(store.load(&token).await.unwrap(), None);
    store.remove(&token).await.unwrap();
}

#[tokio::test]
async fn a_subject_keeps_at_most_the_session_limit() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let mut sessions = Vec::new();
    for _ in 0..MAX_SESSIONS_PER_SUBJECT + 5 {
        sessions.push(store.create("7").await.unwrap());
    }
    assert_eq!(set_size(&store, "7").await, MAX_SESSIONS_PER_SUBJECT);
    let newest = sessions.last().unwrap();
    assert_eq!(
        subject_of(&store, &newest.token).await.as_deref(),
        Some("7")
    );
    store.remove_subject("7").await.unwrap();
}

#[tokio::test]
async fn over_the_limit_expired_keys_go_first_then_the_oldest() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let mut conn = store.conn();
    let gone = store.key(&"b".repeat(64));
    redis::cmd("SADD")
        .arg(store.subject_key("7"))
        .arg(&gone)
        .query_async::<()>(&mut conn)
        .await
        .unwrap();
    let oldest = "c".repeat(64);
    put_raw(&store, &oldest, format!("{}:7", now_secs() + 30), 60).await;
    redis::cmd("SADD")
        .arg(store.subject_key("7"))
        .arg(store.key(&oldest))
        .query_async::<()>(&mut conn)
        .await
        .unwrap();
    for _ in 0..MAX_SESSIONS_PER_SUBJECT - 1 {
        store.create("7").await.unwrap();
    }
    assert_eq!(set_size(&store, "7").await, MAX_SESSIONS_PER_SUBJECT);
    let is_member: bool = redis::cmd("SISMEMBER")
        .arg(store.subject_key("7"))
        .arg(&gone)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(!is_member);
    assert!(subject_of(&store, &oldest).await.is_some());

    store.create("7").await.unwrap();
    assert_eq!(set_size(&store, "7").await, MAX_SESSIONS_PER_SUBJECT);
    assert_eq!(subject_of(&store, &oldest).await, None);
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
async fn cookie_lasts_as_long_as_the_session() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = Session {
        token: "a".repeat(64),
        subject: "7".to_string(),
        expires_in_secs: 9,
    };
    assert!(store.cookie(&session).contains("; Max-Age=9; "));
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
