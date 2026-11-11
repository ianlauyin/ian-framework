use crate::exception;
use crate::exception::Exception;

pub(crate) fn random_token() -> Result<String, Exception> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| {
        exception!(
            code = "AUTH_RANDOM",
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
