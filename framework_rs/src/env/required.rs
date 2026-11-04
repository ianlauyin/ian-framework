use crate::env::{missing, optional};
use crate::exception::Exception;

pub async fn required(name: &str) -> Result<String, Exception> {
    optional(name).await?.ok_or_else(|| missing(name))
}
