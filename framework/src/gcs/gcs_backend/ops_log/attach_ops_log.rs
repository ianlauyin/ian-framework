use crate::logging::LogShipper;

use crate::gcs::gcs_backend::{lock_ops, GcsBackend};

impl GcsBackend {
    pub fn attach_ops_log(&self, shipper: LogShipper) {
        *lock_ops(&self.inner.ops_log) = Some(shipper);
    }
}
