pub(crate) mod error;
pub(crate) mod project;
pub(crate) mod secret_manager;

#[cfg(feature = "gcs")]
pub mod storage;

pub use error::{
    API_FAILED, AUTH_FAILED, CLIENT_BUILD_FAILED, RESOURCE_NOT_FOUND, SERVICE_UNAVAILABLE,
    TRANSPORT_FAILED,
};
pub use project::PROJECT_ID_INVALID;
pub use secret_manager::{
    SECRET_ID_INVALID, SECRET_VALUE_EMPTY, SECRET_VALUE_NOT_UTF8, SecretManager,
};
