use google_cloud_gax::error::rpc::{Code, Status};
use google_cloud_gax::error::Error;

use crate::gcp::error::*;

fn http(status: u16) -> Error {
    Error::http(status, Default::default(), Default::default())
}

fn rpc(code: Code) -> Error {
    Error::service(Status::default().set_code(code))
}

#[test]
fn an_http_status_gives_its_code() {
    for (status, code) in [
        (401, "AUTH_FAILED"),
        (403, "AUTH_FAILED"),
        (404, "RESOURCE_NOT_FOUND"),
        (408, "SERVICE_UNAVAILABLE"),
        (429, "SERVICE_UNAVAILABLE"),
        (500, "SERVICE_UNAVAILABLE"),
        (503, "SERVICE_UNAVAILABLE"),
        (400, "API_FAILED"),
        (412, "API_FAILED"),
    ] {
        assert_eq!(error_code(&http(status)), code, "status {status}");
    }
}

#[test]
fn an_rpc_status_gives_its_code() {
    for (status, code) in [
        (Code::Unauthenticated, "AUTH_FAILED"),
        (Code::PermissionDenied, "AUTH_FAILED"),
        (Code::NotFound, "RESOURCE_NOT_FOUND"),
        (Code::DeadlineExceeded, "SERVICE_UNAVAILABLE"),
        (Code::ResourceExhausted, "SERVICE_UNAVAILABLE"),
        (Code::Unavailable, "SERVICE_UNAVAILABLE"),
        (Code::InvalidArgument, "API_FAILED"),
    ] {
        assert_eq!(error_code(&rpc(status)), code);
    }
}

#[test]
fn a_network_failure_is_a_transport_error() {
    assert_eq!(error_code(&Error::timeout("slow")), "TRANSPORT_FAILED");
    assert_eq!(error_code(&Error::io("reset")), "TRANSPORT_FAILED");
    assert_eq!(error_code(&Error::deser("bad")), "API_FAILED");
}
