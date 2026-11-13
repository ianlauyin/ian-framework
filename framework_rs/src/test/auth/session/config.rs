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
        config_with(|config| config.ttl_secs = MAX_TTL_SECS + 1),
        config_with(|config| config.max_sessions_per_subject = 0),
        config_with(|config| config.ttl_secs = u64::MAX),
        config_with(|config| config.cookie_name = String::new()),
        config_with(|config| config.cookie_name = "a b".to_string()),
        config_with(|config| config.cookie_name = "a;b".to_string()),
        config_with(|config| config.cookie_name = "a=b".to_string()),
        config_with(|config| config.cookie_name = "séance".to_string()),
    ];
    for config in bad {
        let err = config.check().unwrap_err();
        assert_eq!(err.code(), "SESSION_CONFIG_INVALID");
    }
    let edge = config_with(|config| {
        config.ttl_secs = MAX_TTL_SECS;
        config.cookie_name = "__Host-sid".to_string();
    });
    assert!(edge.check().is_ok());
}

#[test]
fn cookie_and_clear_cookie_share_the_attributes() {
    let config = config_with(|config| {
        config.cookie_name = "sid".to_string();
        config.ttl_secs = 3600;
    });
    assert_eq!(
        config.cookie("abc"),
        "sid=abc; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=3600"
    );
    assert_eq!(
        config.clear_cookie(),
        "sid=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0"
    );
}
