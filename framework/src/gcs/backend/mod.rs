mod attach_ops_log;
mod from_adc;
mod new;

use std::fmt;
use std::sync::Arc;

use super::cred::Auth;
use super::token::CachedToken;

#[derive(Clone)]
pub struct GcsBackend {
    inner: Arc<Inner>,
}

impl fmt::Debug for GcsBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcsBackend")
            .field("bucket", &self.inner.bucket)
            .field("email", &self.inner.email)
            .field("auth", &self.inner.auth)
            .finish_non_exhaustive()
    }
}

impl GcsBackend {
    pub fn bucket(&self) -> &str {
        &self.inner.bucket
    }

    pub fn signer_email(&self) -> &str {
        &self.inner.email
    }

    pub(crate) fn ops_shipper(&self) -> Option<crate::logging::LogShipper> {
        lock_ops(&self.inner.ops_log).clone()
    }

    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.inner.http
    }

    pub(crate) fn auth(&self) -> &Auth {
        &self.inner.auth
    }

    pub(crate) fn token_cache(&self) -> &tokio::sync::Mutex<Option<CachedToken>> {
        &self.inner.token
    }
}

struct Inner {
    bucket: String,
    email: String,
    auth: Auth,
    http: reqwest::Client,
    token: tokio::sync::Mutex<Option<CachedToken>>,
    ops_log: std::sync::Mutex<Option<crate::logging::LogShipper>>,
}

fn lock_ops(
    slot: &std::sync::Mutex<Option<crate::logging::LogShipper>>,
) -> std::sync::MutexGuard<'_, Option<crate::logging::LogShipper>> {
    slot.lock().unwrap_or_else(|err| err.into_inner())
}
