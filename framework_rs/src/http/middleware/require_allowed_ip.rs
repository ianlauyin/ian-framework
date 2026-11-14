use std::collections::{BTreeMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::exception::Exception;
use crate::http::error_code;
use crate::http::headers::client_ip;
use crate::logging::Logger;

pub const ALLOWED_IPS_INVALID: &str = "ALLOWED_IPS_INVALID";

#[derive(Clone, Debug)]
pub struct AllowedIps(Arc<HashSet<IpAddr>>);

impl AllowedIps {
    pub fn new(ips: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Self, Exception> {
        let mut set = HashSet::new();
        for ip in ips {
            let text = ip.as_ref().trim();
            let parsed = text.parse::<IpAddr>().map_err(|_| {
                exception!(
                    code = ALLOWED_IPS_INVALID,
                    "allowed ip is not an IP address"
                )
            })?;
            set.insert(parsed);
        }
        Ok(Self(Arc::new(set)))
    }
}

pub async fn require_allowed_ip(
    State(allowed): State<AllowedIps>,
    request: Request,
    next: Next,
) -> Response {
    let ip = client_ip(request.headers());
    let allowed = ip
        .and_then(|value| value.parse::<IpAddr>().ok())
        .is_some_and(|addr| allowed.0.contains(&addr));
    if !allowed {
        Logger::warn(
            "HTTP/ipRejected",
            BTreeMap::from([
                ("method".to_string(), request.method().as_str().into()),
                ("ip".to_string(), ip.unwrap_or("-").into()),
            ]),
        );
        return exception!(code = error_code::FORBIDDEN, "ip not allowed").into_response();
    }
    next.run(request).await
}
