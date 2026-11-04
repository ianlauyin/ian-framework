use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::exception::Exception;

use crate::gcp::credentials::*;

const USER_JSON: &str = r#"{
    "type": "authorized_user",
    "client_id": "id.apps.googleusercontent.com",
    "client_secret": "secret",
    "refresh_token": "refresh"
}"#;

fn credentials_from_json(json: &str) -> Result<Credentials, Exception> {
    credentials_from_value(&serde_json::from_str(json).unwrap())
}

#[test]
fn service_account_key_is_rejected_without_echoing_the_key() {
    let json = r#"{
        "type": "service_account",
        "client_email": "app@example.iam.gserviceaccount.com",
        "private_key": "SECRET-KEY-MATERIAL"
    }"#;
    let err = credentials_from_json(json).unwrap_err();
    assert_eq!(err.code(), "GCP_CONFIG");
    let text = err.to_string();
    assert!(text.contains("service account JSON keys"));
    assert!(!text.contains("SECRET-KEY-MATERIAL"));
}

#[test]
fn impersonated_adc_is_rejected() {
    let json = r#"{
        "type": "impersonated_service_account",
        "service_account_impersonation_url": "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/a@b.iam.gserviceaccount.com:generateAccessToken",
        "source_credentials": {"type": "authorized_user"}
    }"#;
    let err = credentials_from_json(json).unwrap_err();
    assert_eq!(err.code(), "GCP_CONFIG");
    assert!(err.to_string().contains("impersonated"));
    assert!(!err.to_string().contains("generateAccessToken"));
}

#[test]
fn external_account_is_rejected() {
    let json = r#"{"type":"external_account","audience":"x"}"#;
    let err = credentials_from_json(json).unwrap_err();
    assert!(err.to_string().contains("external_account"));
}

#[test]
fn user_adc_reads_the_fields() {
    let Credentials::User(user) = credentials_from_json(USER_JSON).unwrap() else {
        panic!("expected user credentials");
    };
    assert_eq!(user.client_id, "id.apps.googleusercontent.com");
    assert_eq!(user.client_secret, "secret");
    assert_eq!(user.refresh_token, "refresh");
    assert_eq!(user.quota_project_id, None);
    assert!(user.validate().is_ok());
}

#[test]
fn user_adc_keeps_the_quota_project() {
    let json = r#"{
        "type": "authorized_user",
        "client_id": "id",
        "client_secret": "secret",
        "refresh_token": "refresh",
        "quota_project_id": "my-project"
    }"#;
    let Credentials::User(user) = credentials_from_json(json).unwrap() else {
        panic!("expected user credentials");
    };
    assert_eq!(user.quota_project_id.as_deref(), Some("my-project"));
}

#[test]
fn user_adc_needs_each_field() {
    let json = r#"{"type":"authorized_user","client_id":"id","client_secret":"secret"}"#;
    let err = credentials_from_json(json).unwrap_err();
    assert_eq!(err.code(), "GCP_CONFIG");
    assert!(err.to_string().contains("refresh_token"));
}

#[test]
fn incomplete_user_is_rejected() {
    let user = UserCredentials {
        client_id: String::new(),
        client_secret: "secret".to_string(),
        refresh_token: "refresh".to_string(),
        quota_project_id: None,
    };
    assert_eq!(user.validate().unwrap_err().code(), "GCP_CONFIG");
}

#[test]
fn debug_does_not_print_the_secrets() {
    let credentials = credentials_from_json(USER_JSON).unwrap();
    let text = format!("{credentials:?}");
    assert!(text.contains("User"));
    assert!(!text.contains("refresh"));
    assert!(!text.contains("secret"));
    assert!(!text.contains("googleusercontent"));
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ian-framework-test-{}-{name}", std::process::id()))
}

fn write_user_adc(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, USER_JSON).unwrap();
}

fn adc_env(
    credentials: Option<&str>,
    home: Option<&Path>,
    appdata: Option<&Path>,
) -> Result<Credentials, Exception> {
    use std::ffi::OsStr;
    let credentials = credentials.map(OsStr::new);
    crate::test::env::with_vars(
        &[
            ("GOOGLE_APPLICATION_CREDENTIALS", credentials),
            ("HOME", home.map(Path::as_os_str)),
            ("APPDATA", appdata.map(Path::as_os_str)),
        ],
        || crate::test::env::block_on(Credentials::from_adc()),
    )
}

fn response(status: u16, body: &str) -> reqwest::Response {
    let response = axum::http::Response::builder()
        .status(status)
        .body(body.to_string())
        .unwrap();
    reqwest::Response::from(response)
}

fn user() -> UserCredentials {
    UserCredentials {
        client_id: "id".to_string(),
        client_secret: "secret".to_string(),
        refresh_token: "refresh".to_string(),
        quota_project_id: None,
    }
}

#[test]
fn from_adc_reads_the_file_that_the_env_names() {
    let path = temp_path("named/adc.json");
    write_user_adc(&path);
    let credentials = adc_env(path.to_str(), None, None).unwrap();
    assert!(matches!(credentials, Credentials::User(_)));
    assert!(credentials.validate().is_ok());
    assert_eq!(credentials.quota_project(), None);
    let missing = temp_path("named/missing.json");
    let err = adc_env(missing.to_str(), None, None).unwrap_err();
    assert_eq!(err.code(), "GCP_CONFIG");
}

#[test]
fn from_adc_falls_back_to_the_well_known_file_and_then_to_metadata() {
    let home = temp_path("home");
    write_user_adc(&home.join(".config/gcloud/application_default_credentials.json"));
    let credentials = adc_env(Some(""), Some(&home), None).unwrap();
    assert!(matches!(credentials, Credentials::User(_)));

    let appdata = temp_path("appdata");
    write_user_adc(&appdata.join("gcloud/application_default_credentials.json"));
    let credentials = adc_env(None, None, Some(&appdata)).unwrap();
    assert!(matches!(credentials, Credentials::User(_)));

    let empty = temp_path("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let credentials = adc_env(None, Some(&empty), Some(&empty)).unwrap();
    assert!(matches!(credentials, Credentials::Metadata));
    assert!(credentials.validate().is_ok());
}

#[tokio::test]
async fn read_adc_at_rejects_a_missing_large_or_bad_file() {
    let missing = read_adc_at(&temp_path("read/missing.json")).await;
    assert_eq!(missing.unwrap_err().code(), "GCP_CONFIG");

    let large = temp_path("read/large.json");
    std::fs::create_dir_all(large.parent().unwrap()).unwrap();
    std::fs::write(&large, vec![b' '; ADC_FILE_LIMIT as usize + 1]).unwrap();
    let err = read_adc_at(&large).await.unwrap_err();
    assert!(err.to_string().contains("too large"));

    let bad = temp_path("read/bad.json");
    std::fs::write(&bad, "not json").unwrap();
    let err = read_adc_at(&bad).await.unwrap_err();
    assert!(err.to_string().contains("not JSON"));

    let directory = temp_path("read");
    let err = read_adc_at(&directory).await.unwrap_err();
    assert!(err.to_string().contains("cannot read"));
}

#[test]
fn validate_follows_the_credentials_kind() {
    assert!(Credentials::Metadata.validate().is_ok());
    assert!(Credentials::User(user()).validate().is_ok());
    let incomplete = UserCredentials {
        refresh_token: String::new(),
        ..user()
    };
    let err = Credentials::User(incomplete).validate().unwrap_err();
    assert_eq!(err.code(), "GCP_CONFIG");
}

#[tokio::test]
async fn token_from_response_reads_the_token_and_its_expiry() {
    let ok = response(200, r#"{"access_token":"abc","expires_in":3600}"#);
    let token = token_from_response(ok).await.unwrap();
    assert_eq!(token.value, "abc");
    assert!(token.expires_at > Instant::now() + Duration::from_secs(3000));
    let no_expiry = response(200, r#"{"access_token":"abc"}"#);
    let token = token_from_response(no_expiry).await.unwrap();
    assert!(token.expires_at <= Instant::now());
}

#[tokio::test]
async fn token_from_response_rejects_a_bad_response() {
    let denied = response(401, r#"{"error":{"message":"bad grant"}}"#);
    let err = token_from_response(denied).await.err().unwrap();
    assert_eq!(err.code(), "GCP_AUTH");
    assert!(err.to_string().contains("token status 401: bad grant"));
    for body in ["not json", r#"{"access_token":""}"#] {
        let err = token_from_response(response(200, body))
            .await
            .err()
            .unwrap();
        assert_eq!(err.code(), "GCP_AUTH");
    }
}

#[tokio::test]
async fn user_token_is_fetched_with_a_form_post() {
    use crate::test::gcp::{closed_addr, serve};
    let http = crate::gcp::http_client_builder().build().unwrap();
    let body = br#"{"access_token":"abc","expires_in":3600}"#.to_vec();
    let (addr, server) = serve(200, body).await;
    let token = user()
        .fetch_token_at(&http, &format!("http://{addr}/token"))
        .await
        .unwrap();
    assert_eq!(token.value, "abc");
    assert!(server.await.unwrap().starts_with("post /token"));

    let url = format!("http://{}/token", closed_addr());
    let err = user().fetch_token_at(&http, &url).await.err().unwrap();
    assert_eq!(err.code(), "GCP_AUTH");
}
