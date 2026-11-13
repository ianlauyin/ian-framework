use crate::exception;
use crate::exception::Exception;

pub const RANDOM_SOURCE_FAILED: &str = "RANDOM_SOURCE_FAILED";

pub(crate) fn random_token() -> Result<String, Exception> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| {
        exception!(
            code = RANDOM_SOURCE_FAILED,
            format!("os random source failed: {err}"),
        )
    })?;
    Ok(hex::encode(bytes))
}

// A token from `create` is 64 lowercase hex characters. Other values skip the Redis call.
pub(crate) fn is_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
