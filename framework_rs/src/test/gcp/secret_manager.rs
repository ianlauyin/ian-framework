use crate::gcp::Credentials;

use std::ffi::OsStr;

use crate::gcp::credentials::UserCredentials;
use crate::gcp::secret_manager::*;
use crate::test::env::with_var;
use crate::test::gcp::{closed_addr, fake_secret_manager, ok_body, serve, serve_script, Reply};

const SECRET: &str = "MY_SECRET";

fn user(quota_project: Option<&str>) -> Credentials {
    Credentials::User(UserCredentials {
        client_id: "id".to_string(),
        client_secret: "secret".to_string(),
        refresh_token: "refresh".to_string(),
        quota_project_id: quota_project.map(str::to_string),
    })
}

#[test]
fn decode_gives_the_trimmed_value() {
    assert_eq!(
        decode_access_response(200, &ok_body(b"post-key\n")).unwrap(),
        Some("post-key".to_string())
    );
}

#[test]
fn decode_gives_none_for_404() {
    let body = br#"{"error":{"code":404,"message":"Secret [x] not found"}}"#;
    assert_eq!(decode_access_response(404, body).unwrap(), None);
    assert_eq!(decode_access_response(404, b"").unwrap(), None);
}

#[test]
fn decode_rejects_an_empty_value() {
    for value in [&b""[..], b"  \n"] {
        let err = decode_access_response(200, &ok_body(value)).unwrap_err();
        assert_eq!(err.message(), "secret value is empty");
    }
}

#[test]
fn decode_rejects_a_value_that_is_not_utf8() {
    let err = decode_access_response(200, &ok_body(&[0x66, 0x80, 0x6f])).unwrap_err();
    assert_eq!(err.message(), "secret value is not UTF-8");
}

#[test]
fn decode_rejects_a_bad_body_without_echoing_it() {
    let cases: [&[u8]; 4] = [
        b"not json secret-value",
        br#"{"payload":{}}"#,
        br#"{"payload":{"data":12}}"#,
        br#"{"payload":{"data":"secret-value!"}}"#,
    ];
    for body in cases {
        let err = decode_access_response(200, body).unwrap_err();
        assert_eq!(err.code(), "GCP_API");
        assert!(std::error::Error::source(&err).is_none());
        assert!(!format!("{err} {err:?}").contains("secret-value"), "{err}");
    }
}

#[test]
fn decode_reads_the_api_error() {
    let body =
        br#"{"error":{"code":403,"message":"Permission denied","status":"PERMISSION_DENIED"}}"#;
    let err = decode_access_response(403, body).unwrap_err();
    assert_eq!(err.code(), "GCP_AUTH");
    assert_eq!(
        err.message(),
        "secret manager status 403: Permission denied"
    );
    let err = decode_access_response(500, b"<html>down</html>").unwrap_err();
    assert_eq!(err.code(), "GCP_API");
    assert_eq!(err.message(), "secret manager status 500: ");
}

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
    let http = secret_manager_http_client().unwrap();
    let err = access_at(&http, &format!("http://{addr}/v1/x:access"), "token", None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GCP_TRANSPORT");
    let source = std::error::Error::source(&err).unwrap().to_string();
    assert!(!source.contains(&addr.to_string()), "{source}");
}

#[tokio::test]
async fn metadata_project_reads_the_project_id() {
    let (addr, server) = serve(200, b"my-project\n".to_vec()).await;
    let http = secret_manager_http_client().unwrap();
    let url = format!("http://{addr}/computeMetadata/v1/project/project-id");
    let project = metadata_project(&http, &url).await.unwrap();
    assert_eq!(project, "my-project");
    let head = server.await.unwrap();
    assert!(head.contains("metadata-flavor: google"), "{head}");
}

#[tokio::test]
async fn an_unreachable_metadata_server_is_an_error() {
    let http = secret_manager_http_client().unwrap();
    let url = format!("http://{}/project-id", closed_addr());
    let err = metadata_project(&http, &url).await.unwrap_err();
    assert_eq!(err.code(), "GCP_AUTH");
}

#[tokio::test]
async fn a_metadata_server_failure_is_an_error() {
    let (addr, _server) = serve(500, b"down".to_vec()).await;
    let http = secret_manager_http_client().unwrap();
    let err = metadata_project(&http, &format!("http://{addr}/project-id"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GCP_AUTH");
    let (addr, _server) = serve(200, b" ".to_vec()).await;
    let err = metadata_project(&http, &format!("http://{addr}/project-id"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GCP_CONFIG");
}

#[test]
fn user_project_prefers_the_env_var_then_the_quota_project() {
    let read = |env: Option<&str>, quota: Option<&str>| {
        with_var(PROJECT_ENV, env.map(OsStr::new), || {
            user_project(&user(quota))
        })
    };
    assert_eq!(
        read(Some(" env-project "), Some("quota-project")).unwrap(),
        Some("env-project".to_string())
    );
    assert_eq!(
        read(Some(" "), Some("quota-project")).unwrap(),
        Some("quota-project".to_string())
    );
    assert_eq!(read(None, None).unwrap(), None);
    let err = read(Some("bad/project"), None).unwrap_err();
    assert_eq!(err.code(), "GCP_CONFIG");
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

#[tokio::test]
async fn resolve_project_reads_the_metadata_server_or_the_user() {
    let http = secret_manager_http_client().unwrap();
    let (addr, _server) = serve(200, b"my-project".to_vec()).await;
    let url = format!("http://{addr}/project");
    let project = resolve_project(&Credentials::Metadata, &http, &url).await;
    assert_eq!(project.ok().flatten().as_deref(), Some("my-project"));

    let url = format!("http://{}/project", closed_addr());
    let project = resolve_project(&Credentials::Metadata, &http, &url).await;
    assert_eq!(project.unwrap_err().code(), "GCP_AUTH");
}

#[test]
fn resolve_project_uses_the_quota_project_of_a_user() {
    let credentials = user(Some("quota-project"));
    let project = with_var(PROJECT_ENV, None, || {
        let http = secret_manager_http_client().unwrap();
        crate::test::env::block_on(resolve_project(&credentials, &http, "http://unused"))
    });
    assert_eq!(project.ok().flatten().as_deref(), Some("quota-project"));
}
