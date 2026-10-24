use crate::logging::LogShipper;

use crate::gcs::gcs_client::{lock_ops, GcsClient};

impl GcsClient {
    pub fn attach_ops_log(&self, shipper: LogShipper) {
        *lock_ops(&self.inner.ops_log) = Some(shipper);
    }
}
