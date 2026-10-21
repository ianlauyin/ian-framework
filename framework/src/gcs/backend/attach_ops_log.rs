use super::super::GcsBackend;
use super::lock_ops;

impl GcsBackend {
    pub fn attach_ops_log(&self, shipper: crate::logging::LogShipper) {
        *lock_ops(&self.inner.ops_log) = Some(shipper);
    }
}
