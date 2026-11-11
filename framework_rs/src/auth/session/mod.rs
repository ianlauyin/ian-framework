pub(crate) mod config;
pub(crate) mod store;
pub(crate) mod token;

pub use config::SessionConfig;
pub use store::SessionStore;

use std::fmt;

use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;

use crate::exception;
use crate::exception::Exception;
use crate::http::error_code;

#[derive(Clone, PartialEq, Eq)]
pub struct Session {
    pub token: String,
    pub subject: String,
    pub(crate) expires_in_secs: u64,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("subject", &self.subject)
            .field("expires_in_secs", &self.expires_in_secs)
            .finish_non_exhaustive()
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Session {
    type Rejection = Exception;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Exception> {
        let session =
            <Session as OptionalFromRequestParts<S>>::from_request_parts(parts, state).await?;
        session.ok_or_else(|| exception!(code = error_code::UNAUTHORIZED, "unauthorized"))
    }
}

impl<S: Send + Sync> OptionalFromRequestParts<S> for Session {
    type Rejection = Exception;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Option<Self>, Exception> {
        if parts.extensions.get::<LoadFailed>().is_some() {
            return Err(exception!(
                code = "AUTH_STORE",
                "session store command failed"
            ));
        }
        Ok(parts.extensions.get::<Session>().cloned())
    }
}

// `slide_session` inserts this marker when Redis fails, so a handler does not take the failure for a missing session.
#[derive(Clone)]
pub(crate) struct LoadFailed;
