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
use crate::logging::record;
use crate::logging::{LogEvent, LogValue};
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
    let fields = AccessLog::default();
    let pending = Pending::new(&request, fields.clone());
    request.extensions_mut().insert(fields);
    let mut response = next.run(request).await;
    pending.finish(&mut response);
    response
}

struct Pending {
    started: Instant,
    action: String,
    ip: String,
    user_agent: String,
    request_url: String,
    fields: AccessLog,
    recorded: bool,
}

impl Pending {
    fn new(request: &Request, fields: AccessLog) -> Self {
        let path = request_path(request.uri().path());
        let route = request
            .extensions()
            .get::<MatchedPath>()
            .map_or(path, MatchedPath::as_str);
        let request_url = request
            .uri()
            .path_and_query()
            .map_or(path, |url| url.as_str());
        Self {
            started: Instant::now(),
            action: format!("API/{}:{route}", request.method()),
            ip: client_ip(request.headers()).unwrap_or("-").to_string(),
            user_agent: user_agent(request.headers()).unwrap_or("-").to_string(),
            request_url: request_url.to_string(),
            fields,
            recorded: false,
        }
    }

    fn finish(mut self, response: &mut Response) {
        let status = response.status().as_u16();
        let exception = response.extensions_mut().remove::<Arc<Exception>>();
        let action = std::mem::take(&mut self.action);
        self.emit(new_event(action, status, exception.as_deref()));
    }

    fn emit(&mut self, mut event: LogEvent) {
        self.recorded = true;
        let Fields { context, extra } = self.fields.take();
        event.context = context;
        event.extra = extra;
        let reserved = [
            ("ip", &mut self.ip),
            ("user_agent", &mut self.user_agent),
            ("request_url", &mut self.request_url),
        ];
        for (key, value) in reserved {
            event
                .context
                .insert(key.to_string(), std::mem::take(value).into());
        }
        record(event.with_latency(self.started.elapsed()));
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if self.recorded {
            return;
        }
        let action = std::mem::take(&mut self.action);
        self.emit(LogEvent::warn_with(action, CANCELLED));
    }
}

#[derive(Default)]
struct Fields {
    context: BTreeMap<String, LogValue>,
    extra: BTreeMap<String, LogValue>,
}

fn request_path(path: &str) -> &str {
    if path.is_empty() { "-" } else { path }
}

fn new_event(action: String, status: u16, exception: Option<&Exception>) -> LogEvent {
    match (status, exception) {
        (500.., Some(exception)) => LogEvent::error_from(action, exception),
        (500.., None) => LogEvent::error(action, format!("response status {status}")),
        (400.., Some(exception)) => LogEvent::warn_from(action, exception),
        (400.., None) => LogEvent::warn(action),
        _ => LogEvent::info(action),
    }
}
