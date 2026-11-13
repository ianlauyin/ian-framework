use google_cloud_gax::error::Error;

use crate::exception;
use crate::exception::Exception;

pub const API_FAILED: &str = "API_FAILED";
pub const AUTH_FAILED: &str = "AUTH_FAILED";
pub const RESOURCE_NOT_FOUND: &str = "RESOURCE_NOT_FOUND";
pub const SERVICE_UNAVAILABLE: &str = "SERVICE_UNAVAILABLE";
pub const TRANSPORT_FAILED: &str = "TRANSPORT_FAILED";

// The gRPC status codes that the control client can give in place of an HTTP status.
const RPC_NOT_FOUND: i32 = 5;
const RPC_PERMISSION_DENIED: i32 = 7;
const RPC_UNAUTHENTICATED: i32 = 16;
const RPC_DEADLINE_EXCEEDED: i32 = 4;
const RPC_RESOURCE_EXHAUSTED: i32 = 8;
const RPC_UNAVAILABLE: i32 = 14;

pub(crate) fn to_exception(message: &'static str, err: Error) -> Exception {
    exception!(code = error_code(&err), message, source = err)
}

pub(crate) fn error_code(err: &Error) -> &'static str {
    if err.is_authentication() {
        return AUTH_FAILED;
    }
    if let Some(status) = err.http_status_code() {
        return match status {
            401 | 403 => AUTH_FAILED,
            404 => RESOURCE_NOT_FOUND,
            408 | 429 | 500 | 502 | 503 | 504 => SERVICE_UNAVAILABLE,
            _ => API_FAILED,
        };
    }
    if let Some(status) = err.status() {
        return match status.code as i32 {
            RPC_UNAUTHENTICATED | RPC_PERMISSION_DENIED => AUTH_FAILED,
            RPC_NOT_FOUND => RESOURCE_NOT_FOUND,
            RPC_DEADLINE_EXCEEDED | RPC_RESOURCE_EXHAUSTED | RPC_UNAVAILABLE => SERVICE_UNAVAILABLE,
            _ => API_FAILED,
        };
    }
    if err.is_timeout()
        || err.is_transport()
        || err.is_connect()
        || err.is_io()
        || err.is_exhausted()
    {
        return TRANSPORT_FAILED;
    }
    API_FAILED
}
