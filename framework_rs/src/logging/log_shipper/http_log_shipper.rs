use std::sync::Arc;
use std::time::Duration;

use crate::exception;
use crate::exception::Exception;
use crate::logging::log_shipper::LogShipper;
use crate::logging::mask::MASK;
use crate::logging::{LogEvent, LogRequest};

pub(crate) const SHIP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct HttpLogShipper {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) app: &'static str,
    pub(crate) endpoint: String,
    pub(crate) post_api_key: reqwest::header::HeaderValue,
    pub(crate) http: reqwest::Client,
}

impl HttpLogShipper {
    pub fn new(app: &'static str, endpoint: String, post_api_key: &str) -> Result<Self, Exception> {
        let mut post_api_key =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", post_api_key)).map_err(
                |err| {
                    exception!(
                        code = "LOG_CONFIG",
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
                    code = "LOG_CONFIG",
                    "log shipper HTTP client cannot be built",
                    source = err.without_url(),
                )
            })?;
        tracing::info!(app = %app, "log shipping posts to the log server");
        Ok(Self {
            inner: Arc::new(Inner {
                app,
                endpoint,
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
    async fn ship(&self, events: &[LogEvent]) -> bool {
        let log_request = LogRequest {
            events: events.to_vec(),
        };
        let Ok(body) = serde_json::to_string(&log_request) else {
            return false;
        };
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
            Ok(response) => response.status().is_success(),
            Err(_) => false,
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
