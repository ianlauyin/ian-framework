use chrono::{DateTime, Utc};

use crate::auth::session::config::*;

fn config_with(change: impl FnOnce(&mut SessionConfig)) -> SessionConfig {
    let mut config = SessionConfig::default();
    change(&mut config);
    config
}

#[test]
fn checks_the_config() {
    assert!(SessionConfig::default().check().is_ok());
    let bad = [
        config_with(|config| config.ttl_secs = 0),
        config_with(|config| config.max_age_secs = 0),
        config_with(|config| config.max_age_secs = MAX_AGE_LIMIT_SECS + 1),
        config_with(|config| config.ttl_secs = config.max_age_secs + 1),
        config_with(|config| config.ttl_secs = u64::MAX),
        config_with(|config| config.cookie_name = String::new()),
        config_with(|config| config.cookie_name = "a b".to_string()),
        config_with(|config| config.cookie_name = "a;b".to_string()),
        config_with(|config| config.cookie_name = "a=b".to_string()),
        config_with(|config| config.cookie_name = "séance".to_string()),
    ];
    for config in bad {
        let err = config.check().unwrap_err();
        assert_eq!(err.code(), "AUTH_CONFIG");
    }
    let edge = config_with(|config| {
        config.max_age_secs = MAX_AGE_LIMIT_SECS;
        config.ttl_secs = MAX_AGE_LIMIT_SECS;
        config.cookie_name = "__Host-sid".to_string();
    });
    assert!(edge.check().is_ok());
}

#[test]
fn cookie_carries_max_age_and_expires() {
    let config = SessionConfig {
        prefix: "p:".to_string(),
        cookie_name: "sid".to_string(),
        ttl_secs: 3600,
        max_age_secs: 7200,
    };
    let now = DateTime::parse_from_rfc3339("2026-10-24T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(
        config.cookie("abc", 3600, now),
        "sid=abc; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=3600; Expires=Sat, 24 Oct 2026 11:00:00 GMT"
    );
    assert!(config
        .cookie("abc", u64::MAX, now)
        .contains(&format!("; Max-Age={MAX_AGE_LIMIT_SECS}; ")));
}

#[test]
fn clear_cookie_expires_at_once() {
    let config = config_with(|config| config.cookie_name = "my_session".to_string());
    assert_eq!(
        config.clear_cookie(),
        "my_session=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
    );
}
