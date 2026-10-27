use std::env;
use std::time::{Duration, Instant};

use base64::Engine;
use tokio::sync::{Mutex, OnceCell};

use crate::gcp::{
    api_detail, http_client_builder, metadata_get, truncate, CachedToken, Credentials, GcpError,
    GcpErrorKind,
};

const API_ROOT: &str = "https://secretmanager.googleapis.com/v1";
const METADATA_PROJECT_URL: &str = "http://169.254.169.254/computeMetadata/v1/project/project-id";
const PROJECT_ENV: &str = "GOOGLE_CLOUD_PROJECT";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

// None means that the process has no credentials, so each read falls back to the env var.
pub(crate) static SECRET_MANAGER: OnceCell<Option<SecretManager>> = OnceCell::const_new();

// The value is a secret. No error of this file holds the value or a parser error that can quote it.
pub(crate) struct SecretManager {
    pub(crate) http: reqwest::Client,
    pub(crate) api_root: String,
    pub(crate) project: String,
    pub(crate) quota_project: Option<String>,
    pub(crate) credentials: Credentials,
    pub(crate) token: Mutex<CachedToken>,
}

impl SecretManager {
    pub(crate) async fn shared() -> Result<Option<&'static SecretManager>, GcpError> {
        SECRET_MANAGER
            .get_or_try_init(Self::from_adc)
            .await
            .map(Option::as_ref)
    }

    // None means that the secret or its latest version does not exist.
    pub(crate) async fn access_latest(&self, secret: &str) -> Result<Option<String>, GcpError> {
        if !is_name_segment(secret) {
            return Err(GcpError::new(
                GcpErrorKind::Config,
                "secret id has a character that is not valid",
            ));
        }
        let url = format!(
            "{}/projects/{}/secrets/{secret}/versions/latest:access",
            self.api_root, self.project
        );
        let token = self.token().await?;
        access_at(&self.http, &url, &token, self.quota_project.as_deref()).await
    }

    async fn from_adc() -> Result<Option<Self>, GcpError> {
        let credentials = Credentials::from_adc().await?;
        credentials.validate()?;
        let http = http_client()?;
        let project = match &credentials {
            Credentials::Metadata => metadata_project(&http, METADATA_PROJECT_URL).await?,
            Credentials::User(_) => user_project(&credentials)?,
        };
        let Some(project) = project else {
            return Ok(None);
        };
        let token = credentials.fetch_token(&http).await?;
        Ok(Some(Self {
            http,
            api_root: API_ROOT.to_string(),
            project,
            quota_project: credentials.quota_project().map(str::to_string),
            credentials,
            token: Mutex::new(token),
        }))
    }

    async fn token(&self) -> Result<String, GcpError> {
        let mut token = self.token.lock().await;
        if token.expires_at <= Instant::now() {
            *token = self.credentials.fetch_token(&self.http).await?;
        }
        Ok(token.value.clone())
    }
}

// The metadata server is the last credentials source. When it cannot be reached, the process has no credentials.
async fn metadata_project(http: &reqwest::Client, url: &str) -> Result<Option<String>, GcpError> {
    let Ok(response) = metadata_get(http, url).await else {
        return Ok(None);
    };
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(GcpError::new(
            GcpErrorKind::Auth,
            format!("metadata server project status {status}"),
        ));
    }
    let body = response.text().await.map_err(|err| {
        GcpError::with_source(
            GcpErrorKind::Transport,
            "metadata server project is not text",
            err.without_url(),
        )
    })?;
    checked_project(body.trim()).map(Some)
}

fn user_project(credentials: &Credentials) -> Result<Option<String>, GcpError> {
    let from_env = env::var(PROJECT_ENV)
        .ok()
        .filter(|project| !project.trim().is_empty());
    match from_env.as_deref().or(credentials.quota_project()) {
        Some(project) => checked_project(project.trim()).map(Some),
        None => Ok(None),
    }
}

fn checked_project(project: &str) -> Result<String, GcpError> {
    if !is_name_segment(project) {
        return Err(GcpError::new(
            GcpErrorKind::Config,
            "the project id is empty or not valid",
        ));
    }
    Ok(project.to_string())
}

async fn access_at(
    http: &reqwest::Client,
    url: &str,
    token: &str,
    quota_project: Option<&str>,
) -> Result<Option<String>, GcpError> {
    let mut request = http.get(url).bearer_auth(token).timeout(REQUEST_TIMEOUT);
    if let Some(project) = quota_project {
        request = request.header("x-goog-user-project", project);
    }
    let response = request.send().await.map_err(|err| {
        GcpError::with_source(
            GcpErrorKind::Transport,
            "secret manager request failed",
            err.without_url(),
        )
    })?;
    let status = response.status().as_u16();
    let body = response.bytes().await.map_err(|err| {
        GcpError::with_source(
            GcpErrorKind::Transport,
            "secret manager response is not readable",
            err.without_url(),
        )
    })?;
    decode_access_response(status, &body)
}

fn decode_access_response(status: u16, body: &[u8]) -> Result<Option<String>, GcpError> {
    if status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        let detail = api_detail(&String::from_utf8_lossy(body)).unwrap_or_default();
        let kind = match status {
            401 | 403 => GcpErrorKind::Auth,
            _ => GcpErrorKind::Api,
        };
        return Err(GcpError::new(
            kind,
            format!("secret manager status {status}: {}", truncate(&detail, 300)),
        ));
    }
    let parsed: AccessResponse = serde_json::from_slice(body).map_err(|_| {
        GcpError::new(
            GcpErrorKind::Api,
            "secret manager response has no payload data",
        )
    })?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(parsed.payload.data)
        .map_err(|_| GcpError::new(GcpErrorKind::Api, "secret payload is not base64"))?;
    let value = String::from_utf8(bytes)
        .map_err(|_| GcpError::new(GcpErrorKind::Config, "secret value is not UTF-8"))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(GcpError::new(GcpErrorKind::Config, "secret value is empty"));
    }
    Ok(Some(value.to_string()))
}

fn is_name_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn http_client() -> Result<reqwest::Client, GcpError> {
    http_client_builder().build().map_err(|err| {
        GcpError::with_source(GcpErrorKind::Transport, "cannot build the HTTP client", err)
    })
}

#[derive(serde::Deserialize)]
struct AccessResponse {
    payload: Payload,
}

#[derive(serde::Deserialize)]
struct Payload {
    data: String,
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;
    use crate::env::tests::with_var;
    use crate::gcp::credentials::UserCredentials;
    use crate::gcp::tests::{closed_addr, fake_secret_manager, ok_body, serve};

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
            assert_eq!(err.to_string(), "secret value is empty");
        }
    }

    #[test]
    fn decode_rejects_a_value_that_is_not_utf8() {
        let err = decode_access_response(200, &ok_body(&[0x66, 0x80, 0x6f])).unwrap_err();
        assert_eq!(err.to_string(), "secret value is not UTF-8");
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
            assert_eq!(err.kind(), GcpErrorKind::Api);
            assert!(std::error::Error::source(&err).is_none());
            assert!(!format!("{err} {err:?}").contains("secret-value"), "{err}");
        }
    }

    #[test]
    fn decode_reads_the_api_error() {
        let body =
            br#"{"error":{"code":403,"message":"Permission denied","status":"PERMISSION_DENIED"}}"#;
        let err = decode_access_response(403, body).unwrap_err();
        assert_eq!(err.kind(), GcpErrorKind::Auth);
        assert_eq!(
            err.to_string(),
            "secret manager status 403: Permission denied"
        );
        let err = decode_access_response(500, b"<html>down</html>").unwrap_err();
        assert_eq!(err.kind(), GcpErrorKind::Api);
        assert_eq!(err.to_string(), "secret manager status 500: ");
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

    #[tokio::test]
    async fn access_latest_rejects_a_secret_id_that_is_not_valid() {
        let manager = fake_secret_manager(closed_addr());
        for secret in ["", "A/B", "A?B", "A B"] {
            let err = manager.access_latest(secret).await.unwrap_err();
            assert_eq!(err.kind(), GcpErrorKind::Config, "{secret:?}");
        }
    }

    #[tokio::test]
    async fn access_transport_error_omits_the_url() {
        let addr = closed_addr();
        let http = http_client().unwrap();
        let err = access_at(&http, &format!("http://{addr}/v1/x:access"), "token", None)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), GcpErrorKind::Transport);
        let source = std::error::Error::source(&err).unwrap().to_string();
        assert!(!source.contains(&addr.to_string()), "{source}");
    }

    #[tokio::test]
    async fn metadata_project_reads_the_project_id() {
        let (addr, server) = serve(200, b"my-project\n".to_vec()).await;
        let http = http_client().unwrap();
        let url = format!("http://{addr}/computeMetadata/v1/project/project-id");
        let project = metadata_project(&http, &url).await.unwrap();
        assert_eq!(project.as_deref(), Some("my-project"));
        let head = server.await.unwrap();
        assert!(head.contains("metadata-flavor: google"), "{head}");
    }

    #[tokio::test]
    async fn no_metadata_server_is_no_credentials() {
        let http = http_client().unwrap();
        let url = format!("http://{}/project-id", closed_addr());
        assert!(metadata_project(&http, &url).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn a_metadata_server_failure_is_an_error() {
        let (addr, _server) = serve(500, b"down".to_vec()).await;
        let http = http_client().unwrap();
        let err = metadata_project(&http, &format!("http://{addr}/project-id"))
            .await
            .unwrap_err();
        assert_eq!(err.kind(), GcpErrorKind::Auth);
        let (addr, _server) = serve(200, b" ".to_vec()).await;
        let err = metadata_project(&http, &format!("http://{addr}/project-id"))
            .await
            .unwrap_err();
        assert_eq!(err.kind(), GcpErrorKind::Config);
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
        assert_eq!(err.kind(), GcpErrorKind::Config);
    }
}
