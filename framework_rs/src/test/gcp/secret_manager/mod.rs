mod access_response;

use crate::gcp::project::PROJECT_ENV;
use crate::gcp::secret_manager::*;
use crate::test::gcp::{closed_addr, fake_secret_manager, ok_body, serve, serve_script, Reply};

const SECRET: &str = "MY_SECRET";

#[tokio::test]
async fn access_latest_sends_the_name_the_token_and_the_quota_project() {
    let (addr, server) = serve(200, ok_body(b"post-key")).await;
    let manager = fake_secret_manager(addr);
    let value = manager.access_latest(SECRET).await.unwrap();
    assert_eq!(value.as_deref(), Some("post-key"));
    let head = server.await.unwrap();
    assert!(
        head.starts_with(
            "get /v1/projects/my-project/secrets/my_secret/versions/latest:access http/1.1"
        ),
        "{head}"
    );
    assert!(head.contains("authorization: bearer fake-token"), "{head}");
    assert!(head.contains("x-goog-user-project: my-project"), "{head}");
}

#[tokio::test]
async fn access_latest_gives_none_for_404() {
    let (addr, _server) = serve(404, b"{}".to_vec()).await;
    let manager = fake_secret_manager(addr);
    assert_eq!(manager.access_latest(SECRET).await.unwrap(), None);
}

fn ok_reply(value: &[u8]) -> Reply {
    Reply::new(200, &String::from_utf8(ok_body(value)).unwrap())
}

#[tokio::test]
async fn access_latest_fetches_a_new_token_after_one_401() {
    let replies = vec![
        Reply::new(401, "{}"),
        Reply::new(200, r#"{"access_token":"fresh","expires_in":3600}"#),
        ok_reply(b"post-key"),
    ];
    let (addr, server) = serve_script(replies).await;
    let value = fake_secret_manager(addr).access_latest(SECRET).await;
    assert_eq!(value.unwrap().as_deref(), Some("post-key"));
    let requests = server.await.unwrap();
    assert!(
        requests[1].starts_with("GET /token?scopes="),
        "{}",
        requests[1]
    );
    assert!(requests[2]
        .to_ascii_lowercase()
        .contains("authorization: bearer fresh"));
}

#[tokio::test]
async fn access_latest_reports_a_second_401() {
    let replies = vec![
        Reply::new(401, "{}"),
        Reply::new(200, r#"{"access_token":"fresh","expires_in":3600}"#),
        Reply::new(401, "{}"),
    ];
    let (addr, _server) = serve_script(replies).await;
    let err = fake_secret_manager(addr)
        .access_latest(SECRET)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GCP_AUTH");
}

#[tokio::test]
async fn access_latest_retries_a_transient_status_up_to_the_limit() {
    let (addr, _server) = serve_script(vec![Reply::new(503, "{}"), ok_reply(b"v")]).await;
    let value = fake_secret_manager(addr).access_latest(SECRET).await;
    assert_eq!(value.unwrap().as_deref(), Some("v"));

    let replies = (0..ACCESS_ATTEMPTS)
        .map(|_| Reply::new(503, "{}"))
        .collect();
    let (addr, server) = serve_script(replies).await;
    let err = fake_secret_manager(addr)
        .access_latest(SECRET)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GCP_API");
    assert_eq!(server.await.unwrap().len(), ACCESS_ATTEMPTS as usize);
}

#[tokio::test]
async fn access_latest_rejects_a_secret_id_that_is_not_valid() {
    let manager = fake_secret_manager(closed_addr());
    for secret in ["", "A/B", "A?B", "A B"] {
        let err = manager.access_latest(secret).await.unwrap_err();
        assert_eq!(err.code(), "GCP_CONFIG", "{secret:?}");
    }
}

#[tokio::test]
async fn access_transport_error_omits_the_url() {
    let addr = closed_addr();
    let http = crate::gcp::http_client_builder().build().unwrap();
    let err = access_at(&http, &format!("http://{addr}/v1/x:access"), "token", None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GCP_TRANSPORT");
    let source = std::error::Error::source(&err).unwrap().to_string();
    assert!(!source.contains(&addr.to_string()), "{source}");
}

fn temp_adc(name: &str, json: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ian-framework-test-{}-secret-{name}.json",
        std::process::id()
    ));
    std::fs::write(&path, json).unwrap();
    path
}

fn init_from_env(
    path: &std::path::Path,
) -> Result<Option<SecretManager>, crate::exception::Exception> {
    crate::test::env::with_vars(
        &[
            ("GOOGLE_APPLICATION_CREDENTIALS", Some(path.as_os_str())),
            (PROJECT_ENV, None),
        ],
        || crate::test::env::block_on(SecretManager::from_adc()),
    )
}

#[test]
fn from_adc_without_a_project_is_no_credentials() {
    let json = r#"{"type":"authorized_user","client_id":"id","client_secret":"secret","refresh_token":"refresh"}"#;
    let path = temp_adc("no-project", json);
    assert!(matches!(init_from_env(&path), Ok(None)));
}

#[test]
fn from_adc_rejects_a_bad_file_and_incomplete_credentials() {
    let missing = std::env::temp_dir().join("ian-framework-test-secret-missing.json");
    assert!(init_from_env(&missing).is_err());
    let json = r#"{"type":"authorized_user","client_id":"","client_secret":"secret","refresh_token":"refresh"}"#;
    let path = temp_adc("incomplete", json);
    assert!(init_from_env(&path).is_err());
}
