use std::env;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::{metadata_get, read_limited, Credentials};

pub(crate) const METADATA_PROJECT_URL: &str =
    "http://169.254.169.254/computeMetadata/v1/project/project-id";
pub(crate) const PROJECT_ENV: &str = "GOOGLE_CLOUD_PROJECT";

pub(crate) async fn resolve_project(
    credentials: &Credentials,
    http: &reqwest::Client,
    metadata_project_url: &str,
) -> Result<Option<String>, Exception> {
    match credentials {
        Credentials::Metadata => metadata_project(http, metadata_project_url).await.map(Some),
        Credentials::User(_) => user_project(credentials),
    }
}

pub(crate) async fn metadata_project(
    http: &reqwest::Client,
    url: &str,
) -> Result<String, Exception> {
    let response = metadata_get(http, url).await?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(exception!(
            code = "GCP_AUTH",
            format!("metadata server project status {status}"),
        ));
    }
    let body = read_limited(response, "GCP_TRANSPORT", "metadata server project").await?;
    checked_project(String::from_utf8_lossy(&body).trim())
}

pub(crate) fn user_project(credentials: &Credentials) -> Result<Option<String>, Exception> {
    let from_env = env::var(PROJECT_ENV)
        .ok()
        .filter(|project| !project.trim().is_empty());
    match from_env.as_deref().or(credentials.quota_project()) {
        Some(project) => checked_project(project.trim()).map(Some),
        None => Ok(None),
    }
}

pub(crate) fn is_name_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn checked_project(project: &str) -> Result<String, Exception> {
    if !is_name_segment(project) {
        return Err(exception!(
            code = "GCP_CONFIG",
            "the project id is empty or not valid",
        ));
    }
    Ok(project.to_string())
}
