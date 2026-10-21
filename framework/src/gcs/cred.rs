use super::{GcsError, GcsErrorKind};

#[derive(Clone)]
pub(crate) enum Auth {
    Metadata,

    User {
        client_id: String,
        client_secret: String,
        refresh_token: String,
        signer_email: String,
    },
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Auth::Metadata => f.write_str("Metadata"),
            Auth::User { signer_email, .. } => f
                .debug_struct("User")
                .field("signer_email", signer_email)
                .finish_non_exhaustive(),
        }
    }
}

pub(crate) fn validate_auth(auth: &Auth) -> Result<(), GcsError> {
    let Auth::User {
        client_id,
        client_secret,
        refresh_token,
        signer_email,
    } = auth
    else {
        return Ok(());
    };
    if client_id.is_empty() || client_secret.is_empty() || refresh_token.is_empty() {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "user credentials are incomplete",
        ));
    }
    validate_signer_email(signer_email)
}

pub(crate) fn validate_signer_email(email: &str) -> Result<(), GcsError> {
    if email.is_empty()
        || !email.contains('@')
        || email.contains(char::is_whitespace)
        || email.contains(['\r', '\n'])
    {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "signer email is not a service account email",
        ));
    }
    Ok(())
}
