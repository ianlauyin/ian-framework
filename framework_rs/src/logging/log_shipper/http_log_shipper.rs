use std::sync::Arc;
use std::time::Duration;

use crate::exception;
use crate::exception::Exception;
use crate::logging::LogEvent;
use crate::logging::log_shipper::LogShipper;
use crate::logging::mask::MASK;

pub const API_KEY_HEADER_INVALID: &str = "API_KEY_HEADER_INVALID";
pub const HTTP_CLIENT_BUILD_FAILED: &str = "HTTP_CLIENT_BUILD_FAILED";
pub const LOG_ENCODE_FAILED: &str = "LOG_ENCODE_FAILED";
pub const LOG_SHIP_FAILED: &str = "LOG_SHIP_FAILED";

pub const SHIP_CHUNK: usize = 1_000;

const SHIP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct HttpLogShipper {
    inner: Arc<Inner>,
}

impl HttpLogShipper {
    pub fn new(app: &'static str, endpoint: String, post_api_key: &str) -> Result<Self, Exception> {
        let mut post_api_key =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", post_api_key)).map_err(
                |err| {
                    exception!(
                        code = API_KEY_HEADER_INVALID,
                        "post API key is not a valid header value",
                        source = err,
                    )
                },
            )?;
        post_api_key.set_sensitive(true);
        let http = reqwest::Client::builder()
            .connect_timeout(SHIP_TIMEOUT)
            .timeout(SHIP_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|err| {
                exception!(
                    code = HTTP_CLIENT_BUILD_FAILED,
                    "log shipper HTTP client cannot be built",
                    source = err.without_url(),
                )
            })?;
        tracing::info!(app = %app, "log shipping posts to the log server");
        Ok(Self {
            inner: Arc::new(Inner {
                app,
                endpoint: endpoint.trim_end_matches('/').to_string(),
                post_api_key,
                http,
            }),
        })
    }

    fn batch_url(&self) -> String {
        format!("{}/log/{}", self.inner.endpoint, self.inner.app)
    }
}

impl LogShipper for HttpLogShipper {
    async fn ship(&self, events: &[LogEvent]) -> Result<(), Exception> {
        let body = serde_json::to_string(&Batch { events }).map_err(|err| {
            exception!(
                code = LOG_ENCODE_FAILED,
                "log batch is not json",
                source = err
            )
        })?;
        let request = self
            .inner
            .http
            .post(self.batch_url())
            .timeout(SHIP_TIMEOUT)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(
                reqwest::header::AUTHORIZATION,
                self.inner.post_api_key.clone(),
            );
        match request.body(body).send().await {
            Ok(response) if response.status().is_success() => Ok(()),
            Ok(_) => Err(exception!(
                code = LOG_SHIP_FAILED,
                "log server refused the batch"
            )),
            Err(err) => Err(exception!(
                code = LOG_SHIP_FAILED,
                "cannot reach the log server",
                source = err,
            )),
        }
    }
}

impl std::fmt::Debug for HttpLogShipper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpLogShipper")
            .field("app", &self.inner.app)
            .field("endpoint", &self.inner.endpoint)
            .field("post_api_key", &MASK)
            .finish_non_exhaustive()
    }
}

struct Inner {
    app: &'static str,
    endpoint: String,
    post_api_key: reqwest::header::HeaderValue,
    http: reqwest::Client,
}

#[derive(serde::Serialize)]
struct Batch<'a> {
    events: &'a [LogEvent],
}
