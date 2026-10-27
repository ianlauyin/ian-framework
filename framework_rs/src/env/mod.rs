mod error;
mod optional;
mod required;
mod u64_or;

pub use error::{EnvError, EnvErrorKind};
pub use optional::optional;
pub use required::required;
pub use u64_or::u64_or;

#[cfg(test)]
pub(crate) mod tests {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::sync::{Mutex, MutexGuard};

    pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|err| err.into_inner())
    }

    pub(crate) fn not_unicode() -> &'static OsStr {
        OsStr::from_bytes(&[0x66, 0x6f, 0x80])
    }

    pub(crate) fn with_var<T>(name: &str, value: Option<&OsStr>, read: impl FnOnce() -> T) -> T {
        with_vars(&[(name, value)], read)
    }

    pub(crate) fn with_vars<T>(vars: &[(&str, Option<&OsStr>)], read: impl FnOnce() -> T) -> T {
        let _guard = env_lock();
        let previous: Vec<_> = vars
            .iter()
            .map(|(name, _)| (*name, std::env::var_os(name)))
            .collect();
        for (name, value) in vars {
            set_or_remove(name, *value);
        }
        let result = read();
        for (name, value) in &previous {
            set_or_remove(name, value.as_deref());
        }
        result
    }

    pub(crate) fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        #[cfg(feature = "gcp")]
        crate::gcp::tests::no_secret_manager();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }

    fn set_or_remove(name: &str, value: Option<&OsStr>) {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}
