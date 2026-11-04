use crate::gcp::{Credentials, UserCredentials};

use crate::gcs::gcs_client::credentials::auth::*;
use crate::test::gcp::closed_addr;
use crate::test::gcs::gcs_client::{serve_script, Reply};

fn user_credentials() -> Credentials {
    Credentials::User(UserCredentials {
        client_id: "id.apps.googleusercontent.com".to_string(),
        client_secret: "secret".to_string(),
        refresh_token: "refresh".to_string(),
        quota_project_id: None,
    })
}

#[test]
fn metadata_credentials_need_no_signer_email() {
    let auth = auth_from_credentials(Credentials::Metadata, None).unwrap();
    assert!(matches!(auth, Auth::Metadata));
}

#[test]
fn user_adc_requires_a_signer_email() {
    let err = auth_from_credentials(user_credentials(), None).unwrap_err();
    assert_eq!(err.code(), "GCS_CONFIG");
    assert!(err
        .to_string()
        .contains("user application default credentials require a signer email"));
}

#[test]
fn user_adc_keeps_the_signer_email() {
    let auth = auth_from_credentials(
        user_credentials(),
        Some("signer@example.iam.gserviceaccount.com"),
    )
    .unwrap();
    match auth {
        Auth::User {
            credentials,
            signer_email,
        } => {
            assert_eq!(credentials.client_id, "id.apps.googleusercontent.com");
            assert_eq!(signer_email, "signer@example.iam.gserviceaccount.com");
        }
        Auth::Metadata => panic!("expected user auth"),
    }
}

#[test]
fn debug_does_not_print_the_refresh_token() {
    let auth = auth_from_credentials(
        user_credentials(),
        Some("signer@example.iam.gserviceaccount.com"),
    )
    .unwrap();
    let text = format!("{auth:?}");
    assert!(text.contains("signer@example.iam.gserviceaccount.com"));
    assert!(!text.contains("refresh"));
    assert!(!text.contains("secret"));
}

fn http() -> reqwest::Client {
    reqwest::Client::new()
}

#[tokio::test]
async fn metadata_email_reads_the_trimmed_body() {
    let (addr, server) = serve_script(vec![Reply::new(200, " signer@example.com\n")]).await;
    let url = format!("http://{addr}/email");
    assert_eq!(
        metadata_email(&http(), &url).await.unwrap(),
        "signer@example.com"
    );
    let request = server.await.unwrap().remove(0).to_ascii_lowercase();
    assert!(request.contains("metadata-flavor: google"), "{request}");
}

#[tokio::test]
async fn metadata_email_refuses_a_failed_status_and_a_bad_email() {
    let replies = vec![Reply::new(500, "boom"), Reply::new(200, "not an email")];
    let (addr, _server) = serve_script(replies).await;
    let url = format!("http://{addr}/email");
    let err = metadata_email(&http(), &url).await.unwrap_err();
    assert_eq!(err.code(), "GCS_AUTH");
    assert!(err.to_string().contains("status 500: boom"), "{err}");
    let err = metadata_email(&http(), &url).await.unwrap_err();
    assert_eq!(err.code(), "GCS_CONFIG");
}

#[tokio::test]
async fn metadata_email_reports_an_unreachable_server() {
    let url = format!("http://{}/email", closed_addr());
    let err = metadata_email(&http(), &url).await.unwrap_err();
    assert_eq!(err.code(), "GCS_AUTH");
}
