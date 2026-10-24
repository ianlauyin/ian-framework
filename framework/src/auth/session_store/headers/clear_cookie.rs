use crate::auth::session_store::SessionStore;

impl SessionStore {
    pub fn clear_cookie(&self) -> String {
        clear_value(&self.inner.config.cookie_name)
    }
}

fn clear_value(cookie_name: &str) -> String {
    format!(
        "{cookie_name}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_cookie_expires_at_once() {
        assert_eq!(
            clear_value("ics_session"),
            "ics_session=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
        );
    }
}
