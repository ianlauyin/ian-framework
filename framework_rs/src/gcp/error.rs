use google_cloud_gax::error::Error;

use crate::exception;
use crate::exception::Exception;

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
        return "GCP_AUTH";
    }
    if let Some(status) = err.http_status_code() {
        return match status {
            401 | 403 => "GCP_AUTH",
            404 => "GCS_NOT_FOUND",
            408 | 429 | 500 | 502 | 503 | 504 => "GCS_UNAVAILABLE",
            _ => "GCP_API",
        };
    }
    if let Some(status) = err.status() {
        return match status.code as i32 {
            RPC_UNAUTHENTICATED | RPC_PERMISSION_DENIED => "GCP_AUTH",
            RPC_NOT_FOUND => "GCS_NOT_FOUND",
            RPC_DEADLINE_EXCEEDED | RPC_RESOURCE_EXHAUSTED | RPC_UNAVAILABLE => "GCS_UNAVAILABLE",
            _ => "GCP_API",
        };
    }
    if err.is_timeout()
        || err.is_transport()
        || err.is_connect()
        || err.is_io()
        || err.is_exhausted()
    {
        return "GCP_TRANSPORT";
    }
    "GCP_API"
}
