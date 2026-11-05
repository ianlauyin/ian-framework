// RFC 9110 token68: one or more of ALPHA DIGIT "-" "." "_" "~" "+" "/", then any "=".
pub fn is_token68(token: &str) -> bool {
    let body = token.trim_end_matches('=');
    !body.is_empty()
        && body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/".contains(&byte))
}
