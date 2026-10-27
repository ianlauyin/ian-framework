mod error;
mod required;
mod u64_or;

pub use error::{EnvError, EnvErrorKind};
pub use required::required;
pub use u64_or::u64_or;

#[cfg(test)]
mod tests {
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
        let _guard = env_lock();
        let previous = std::env::var_os(name);
        set_or_remove(name, value);
        let result = read();
        set_or_remove(name, previous.as_deref());
        result
    }

    fn set_or_remove(name: &str, value: Option<&OsStr>) {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}
