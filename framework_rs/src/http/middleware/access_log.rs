use std::collections::BTreeMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use axum::extract::{FromRequestParts, MatchedPath, Request};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;

use crate::exception::Exception;
use crate::http::headers::client::{client_ip, user_agent};
use crate::logging::{LogEvent, LogValue, Logger, with_log};
use crate::mutex;

pub const CANCELLED: &str = "request was cancelled";

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
        mutex::lock(&self.fields)
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

pub async fn access_log(mut request: Request, next: Next) -> Response {
    if Logger::get().is_none() {
        return next.run(request).await;
    }
    let method = request.method().as_str().to_string();
    let path = request_path(request.uri().path());
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| path.clone(), |matched| matched.as_str().to_string());
    let ip = client_ip(request.headers()).unwrap_or("-").to_string();
    let user_agent = user_agent(request.headers()).unwrap_or("-").to_string();
    let request_url = request
        .uri()
        .path_and_query()
        .map_or_else(|| path.clone(), |url| url.as_str().to_string());

    let fields = AccessLog::default();
    request.extensions_mut().insert(fields.clone());
    let mut pending = Pending {
        started: Instant::now(),
        entry: Some(Entry {
            action: action(&method, &route),
            ip,
            user_agent,
            request_url,
            fields,
        }),
    };
    with_log(next.run(request), move |response| {
        let status = response.status().as_u16();
        let detail = response
            .extensions_mut()
            .remove::<Arc<Exception>>()
            .map(|exception| format!("{exception}\n{}", String::from(exception.as_ref())));
        match pending.entry.take() {
            Some(entry) => entry.event(|action| new_event(action, status, detail)),
            None => new_event("-".to_string(), status, detail),
        }
    })
    .await
}

struct Pending {
    started: Instant,
    entry: Option<Entry>,
}

impl Drop for Pending {
    fn drop(&mut self) {
        let Some(entry) = self.entry.take() else {
            return;
        };
        let Some(logger) = Logger::get() else {
            return;
        };
        let mut event = entry.event(|action| LogEvent::warn_with(action, CANCELLED));
        event.latency_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        logger.record(event);
    }
}

struct Entry {
    action: String,
    ip: String,
    user_agent: String,
    request_url: String,
    fields: AccessLog,
}

impl Entry {
    fn event(self, build: impl FnOnce(String) -> LogEvent) -> LogEvent {
        let mut event = build(self.action);
        let Fields { context, extra } = self.fields.take();
        event.context = context;
        event.extra = extra;
        event.context.insert("ip".to_string(), self.ip.into());
        event
            .context
            .insert("user_agent".to_string(), self.user_agent.into());
        event
            .context
            .insert("request_url".to_string(), self.request_url.into());
        event
    }
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

fn new_event(action: String, status: u16, detail: Option<String>) -> LogEvent {
    if status >= 500 {
        LogEvent::error(
            action,
            detail.unwrap_or_else(|| format!("response status {status}")),
        )
    } else if status >= 400 {
        match detail {
            Some(detail) => LogEvent::warn_with(action, detail),
            None => LogEvent::warn(action),
        }
    } else {
        LogEvent::info(action)
    }
}
