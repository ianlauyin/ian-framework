use std::env;
use std::time::Duration;

use crate::exception;
use crate::exception::Exception;

const METADATA_PROJECT_URL: &str = "http://169.254.169.254/computeMetadata/v1/project/project-id";
// `GOOGLE_CLOUD_PROJECT` is a Google Cloud standard variable, not an app setting.
const PROJECT_ENV: &str = "GOOGLE_CLOUD_PROJECT";
const METADATA_TIMEOUT: Duration = Duration::from_secs(2);

// None means that the process has no project: no variable and no metadata server.
pub(crate) async fn resolve_project() -> Result<Option<String>, Exception> {
    let project = match env::var(PROJECT_ENV) {
        Ok(project) if !project.trim().is_empty() => project,
        _ => match fetch_metadata_project().await {
            Some(project) => project,
            None => return Ok(None),
        },
    };
    let project = project.trim();
    if !is_safe_name(project) {
        return Err(exception!(
            code = "GCP_CONFIG",
            "the project id is empty or not valid",
        ));
    }
    Ok(Some(project.to_string()))
}

pub(crate) fn is_safe_name(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

async fn fetch_metadata_project() -> Option<String> {
    let response = reqwest::Client::new()
        .get(METADATA_PROJECT_URL)
        .header("Metadata-Flavor", "Google")
        .timeout(METADATA_TIMEOUT)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().await.ok()
}
