use std::collections::BTreeMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;

use crate::http::headers::client::{client_ip, user_agent};
use crate::logging::{LogEvent, LogValue, Logger};

#[derive(Clone, Default)]
pub struct AccessLog {
    fields: Arc<Mutex<Fields>>,
}

impl AccessLog {
    pub fn context(&self, key: impl Into<String>, value: impl Into<LogValue>) {
        self.lock().context.insert(key.into(), value.into());
    }

    pub fn extra(&self, key: impl Into<String>, value: impl Into<LogValue>) {
        self.lock().extra.insert(key.into(), value.into());
    }

    fn lock(&self) -> MutexGuard<'_, Fields> {
        self.fields.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn take(&self) -> Fields {
        std::mem::take(&mut *self.lock())
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AccessLog {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Infallible> {
        Ok(parts
            .extensions
            .get::<AccessLog>()
            .cloned()
            .unwrap_or_default())
    }
}

pub async fn access_log(
    State(logger): State<Logger>,
    mut request: Request,
    next: Next,
) -> Response {
    let method = request.method().as_str().to_string();
    let path = request_path(request.uri().path());
    let ip = client_ip(request.headers()).unwrap_or("-").to_string();
    let user_agent = user_agent(request.headers()).unwrap_or("-").to_string();
    let request_url = request
        .uri()
        .path_and_query()
        .map_or_else(|| path.clone(), |url| url.as_str().to_string());

    let fields = AccessLog::default();
    request.extensions_mut().insert(fields.clone());
    let started = Instant::now();
    let response = next.run(request).await;
    let status = response.status().as_u16();
    let mut event = new_event(action(&method, &path), status);
    let Fields { context, extra } = fields.take();
    event.context = context;
    event.extra = extra;
    // The middleware keys go in last, so a handler cannot replace them.
    event.context.insert("ip".to_string(), ip.into());
    event
        .context
        .insert("user_agent".to_string(), user_agent.into());
    event
        .context
        .insert("request_url".to_string(), request_url.into());
    event.latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    logger.record(event);
    response
}

#[derive(Default)]
struct Fields {
    context: BTreeMap<String, LogValue>,
    extra: BTreeMap<String, LogValue>,
}

fn action(method: &str, path: &str) -> String {
    if method.is_empty() {
        return "-".to_string();
    }
    let path = if path.is_empty() { "/" } else { path };
    format!("API/{method}:{path}")
}

fn request_path(path: &str) -> String {
    if path.is_empty() {
        "-".to_string()
    } else {
        path.to_string()
    }
}

fn new_event(action: String, status: u16) -> LogEvent {
    if status >= 500 {
        LogEvent::error(action, format!("response status {status}"))
    } else if status >= 400 {
        LogEvent::warn(action)
    } else {
        LogEvent::info(action)
    }
}
