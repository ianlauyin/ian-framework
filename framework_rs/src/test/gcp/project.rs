use std::ffi::OsStr;

use crate::gcp::credentials::UserCredentials;
use crate::gcp::project::*;
use crate::gcp::Credentials;
use crate::test::env::with_var;
use crate::test::gcp::{closed_addr, serve};

fn user(quota_project: Option<&str>) -> Credentials {
    Credentials::User(UserCredentials {
        client_id: "id".to_string(),
        client_secret: "secret".to_string(),
        refresh_token: "refresh".to_string(),
        quota_project_id: quota_project.map(str::to_string),
    })
}

#[tokio::test]
async fn metadata_project_reads_the_project_id() {
    let (addr, server) = serve(200, b"my-project\n".to_vec()).await;
    let http = crate::gcp::http_client_builder().build().unwrap();
    let url = format!("http://{addr}/computeMetadata/v1/project/project-id");
    let project = metadata_project(&http, &url).await.unwrap();
    assert_eq!(project, "my-project");
    let head = server.await.unwrap();
    assert!(head.contains("metadata-flavor: google"), "{head}");
}

#[tokio::test]
async fn an_unreachable_metadata_server_is_an_error() {
    let http = crate::gcp::http_client_builder().build().unwrap();
    let url = format!("http://{}/project-id", closed_addr());
    let err = metadata_project(&http, &url).await.unwrap_err();
    assert_eq!(err.code(), "GCP_AUTH");
}

#[tokio::test]
async fn a_metadata_server_failure_is_an_error() {
    let (addr, _server) = serve(500, b"down".to_vec()).await;
    let http = crate::gcp::http_client_builder().build().unwrap();
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

#[tokio::test]
async fn resolve_project_reads_the_metadata_server_or_the_user() {
    let http = crate::gcp::http_client_builder().build().unwrap();
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
        let http = crate::gcp::http_client_builder().build().unwrap();
        crate::test::env::block_on(resolve_project(&credentials, &http, "http://unused"))
    });
    assert_eq!(project.ok().flatten().as_deref(), Some("quota-project"));
}
