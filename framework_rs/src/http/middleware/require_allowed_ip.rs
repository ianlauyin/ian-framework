use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::exception::Exception;
use crate::http::error_code;
use crate::http::headers::client_ip;

pub const ALLOWED_IPS_INVALID: &str = "ALLOWED_IPS_INVALID";

#[derive(Clone, Debug)]
pub struct AllowedIps(Arc<[IpRange]>);

impl AllowedIps {
    pub fn new(ips: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Self, Exception> {
        let ranges = ips
            .into_iter()
            .map(|ip| IpRange::parse(ip.as_ref().trim()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self(ranges.into()))
    }

    fn contains(&self, addr: IpAddr) -> bool {
        self.0.iter().any(|range| range.contains(addr))
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
        .is_some_and(|addr| allowed.contains(addr));
    if !allowed {
        return exception!(code = error_code::FORBIDDEN, "ip not allowed").into_response();
    }
    next.run(request).await
}

#[derive(Clone, Copy, Debug)]
struct IpRange {
    v4: bool,
    network: u128,
    host: u128,
}

impl IpRange {
    fn parse(text: &str) -> Result<Self, Exception> {
        let (addr, prefix) = match text.split_once('/') {
            Some((addr, prefix)) => (addr, Some(prefix)),
            None => (text, None),
        };
        let addr = addr.parse::<IpAddr>().map_err(|err| {
            exception!(
                code = ALLOWED_IPS_INVALID,
                "allowed ip is not an IP address",
                source = err
            )
        })?;
        let (v4, value, bits) = split(addr);
        let prefix = match prefix {
            None => bits,
            Some(p) => p
                .parse::<u32>()
                .ok()
                .filter(|n| p.bytes().all(|b| b.is_ascii_digit()) && *n <= bits)
                .ok_or_else(|| {
                    exception!(
                        code = ALLOWED_IPS_INVALID,
                        "allowed ip range prefix is invalid"
                    )
                })?,
        };
        let host = (u128::MAX >> (128 - bits)).checked_shr(prefix).unwrap_or(0);
        if value & host != 0 {
            return Err(exception!(
                code = ALLOWED_IPS_INVALID,
                "allowed ip range has host bits set"
            ));
        }
        Ok(Self {
            v4,
            network: value,
            host,
        })
    }

    fn contains(&self, addr: IpAddr) -> bool {
        let (v4, value, _) = split(addr);
        v4 == self.v4 && value & !self.host == self.network
    }
}

fn split(addr: IpAddr) -> (bool, u128, u32) {
    match addr {
        IpAddr::V4(a) => (true, u128::from(u32::from(a)), 32),
        IpAddr::V6(a) => (false, u128::from(a), 128),
    }
}
