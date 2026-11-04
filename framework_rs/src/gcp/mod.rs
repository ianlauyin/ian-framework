pub(crate) mod credentials;
pub(crate) mod error;
pub(crate) mod http_client;
pub(crate) mod secret_manager;

pub(crate) use credentials::{metadata_get, CachedToken, Credentials};
pub(crate) use error::{api_detail, truncate};
pub(crate) use http_client::http_client_builder;
pub(crate) use secret_manager::SecretManager;

#[cfg(feature = "gcs")]
pub(crate) use credentials::{metadata_token, UserCredentials, METADATA_TOKEN_URL};
