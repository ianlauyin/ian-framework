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
        (401, "GCP_AUTH"),
        (403, "GCP_AUTH"),
        (404, "GCS_NOT_FOUND"),
        (408, "GCS_UNAVAILABLE"),
        (429, "GCS_UNAVAILABLE"),
        (500, "GCS_UNAVAILABLE"),
        (503, "GCS_UNAVAILABLE"),
        (400, "GCP_API"),
        (412, "GCP_API"),
    ] {
        assert_eq!(error_code(&http(status)), code, "status {status}");
    }
}

#[test]
fn an_rpc_status_gives_its_code() {
    for (status, code) in [
        (Code::Unauthenticated, "GCP_AUTH"),
        (Code::PermissionDenied, "GCP_AUTH"),
        (Code::NotFound, "GCS_NOT_FOUND"),
        (Code::DeadlineExceeded, "GCS_UNAVAILABLE"),
        (Code::ResourceExhausted, "GCS_UNAVAILABLE"),
        (Code::Unavailable, "GCS_UNAVAILABLE"),
        (Code::InvalidArgument, "GCP_API"),
    ] {
        assert_eq!(error_code(&rpc(status)), code);
    }
}

#[test]
fn a_network_failure_is_a_transport_error() {
    assert_eq!(error_code(&Error::timeout("slow")), "GCP_TRANSPORT");
    assert_eq!(error_code(&Error::io("reset")), "GCP_TRANSPORT");
    assert_eq!(error_code(&Error::deser("bad")), "GCP_API");
}
