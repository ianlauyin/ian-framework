use std::fmt;

use crate::http::IntoApiError;

macro_rules! library_codes {
    ($($name:ident = $text:literal, $status:literal, $message:literal;)*) => {
        pub mod code {
            $(pub const $name: &str = $text;)*
        }

        // Each library code gives its HTTP status and its client message. A new code needs both.
        const LIBRARY_CODES: &[(&str, u16, &str)] = &[$(($text, $status, $message)),*];
    };
}

library_codes! {
    EXTERNAL = "external", 500, "internal error";
    ENV_MISSING = "env_missing", 500, "internal error";
    ENV_INVALID = "env_invalid", 500, "internal error";
    ENV_NOT_UNICODE = "env_not_unicode", 500, "internal error";
    ENV_SECRET = "env_secret", 500, "internal error";
    AUTH_CONFIG = "auth_config", 500, "session store is not available";
    AUTH_STORE = "auth_store", 503, "session store is not available";
    AUTH_PASSWORD = "auth_password", 500, "password hash failed";
    AUTH_RANDOM = "auth_random", 500, "session token failed";
    GCP_AUTH = "gcp_auth", 500, "internal error";
    GCP_CONFIG = "gcp_config", 500, "internal error";
    GCP_API = "gcp_api", 500, "internal error";
    GCP_TRANSPORT = "gcp_transport", 500, "internal error";
    GCS_AUTH = "gcs_auth", 500, "storage is not available";
    GCS_CONFIG = "gcs_config", 500, "storage is not available";
    GCS_API = "gcs_api", 502, "storage request failed";
    GCS_TRANSPORT = "gcs_transport", 502, "storage request failed";
    GCS_LOCAL = "gcs_local", 500, "storage request failed";
    GCS_NOT_FOUND = "gcs_not_found", 404, "storage object not found";
}

// A string literal as the code is a project code. A project code must not be a code of the library.
#[macro_export]
macro_rules! exception {
    (@build $code:expr, $message:expr $(,)?) => {
        $crate::exception::Exception::__new($code, $message, file!(), line!())
    };
    (@build $code:expr, $message:expr, status = $status:expr $(,)?) => {
        $crate::exception::Exception::__with_status($code, $status, $message, file!(), line!())
    };
    (@build $code:expr, $message:expr, caused_by = $source:expr $(,)?) => {
        $crate::exception::Exception::__caused_by($code, $message, $source, file!(), line!())
    };
    (@build $code:expr, $message:expr, source = $source:expr $(,)?) => {
        $crate::exception::Exception::__with_source($code, $message, $source, file!(), line!())
    };
    ($code:literal, $($rest:tt)*) => {{
        $crate::exception::Exception::__assert_project_code($code);
        $crate::exception!(@build $code, $($rest)*)
    }};
    ($code:expr, $($rest:tt)*) => {
        $crate::exception!(@build $code, $($rest)*)
    };
}

#[derive(Clone)]
pub struct Exception {
    code: &'static str,
    status: Option<u16>,
    message: String,
    location: Option<Location>,
    source: Option<Box<Exception>>,
}

impl Exception {
    #[doc(hidden)]
    pub fn __assert_project_code(code: &str) {
        debug_assert!(
            library_entry(code).is_none(),
            "the code {code} belongs to the library"
        );
    }

    #[doc(hidden)]
    pub fn __new(
        code: &'static str,
        message: impl Into<String>,
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            code,
            status: None,
            message: message.into(),
            location: Some(Location { file, line }),
            source: None,
        }
    }

    #[doc(hidden)]
    pub fn __with_status(
        code: &'static str,
        status: u16,
        message: impl Into<String>,
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            status: Some(status),
            ..Self::__new(code, message, file, line)
        }
    }

    #[doc(hidden)]
    pub fn __caused_by(
        code: &'static str,
        message: impl Into<String>,
        source: Exception,
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            source: Some(Box::new(source)),
            ..Self::__new(code, message, file, line)
        }
    }

    #[doc(hidden)]
    pub fn __with_source<E: std::error::Error + 'static>(
        code: &'static str,
        message: impl Into<String>,
        source: E,
        file: &'static str,
        line: u32,
    ) -> Self {
        let source: &(dyn std::error::Error + 'static) = &source;
        let cause = match source.downcast_ref::<Exception>() {
            Some(exception) => exception.clone(),
            None => Self::from_error(source, Some(std::any::type_name::<E>())),
        };
        Self::__caused_by(code, message, cause, file, line)
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn status(&self) -> Option<u16> {
        self.status
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    // The file and the line of the `exception!` call. An `external` exception has none.
    pub fn location(&self) -> Option<(&'static str, u32)> {
        self.location.map(|location| (location.file, location.line))
    }

    pub fn source_exception(&self) -> Option<&Exception> {
        self.source.as_deref()
    }

    pub fn with_code(self, code: &'static str) -> Self {
        Self { code, ..self }
    }

    pub fn with_status(self, status: Option<u16>) -> Self {
        Self { status, ..self }
    }

    // The trace starts at this exception and ends at the root source.
    pub fn trace(&self) -> impl Iterator<Item = &Exception> {
        std::iter::successors(Some(self), |exception| exception.source_exception())
    }

    // The message has only a type name and an `io::ErrorKind`, never the text of the source.
    fn from_error(error: &(dyn std::error::Error + 'static), type_name: Option<&str>) -> Self {
        Self {
            code: code::EXTERNAL,
            status: None,
            message: external_message(error, type_name),
            location: None,
            source: error
                .source()
                .map(|source| Box::new(Self::from_error(source, None))),
        }
    }
}

impl fmt::Debug for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exception")
            .field("code", &self.code)
            .field("message", &self.message)
            .field("status", &self.status)
            .field("location", &self.location())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl PartialEq for Exception {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code && self.message == other.message && self.status == other.status
    }
}

impl Eq for Exception {}

impl std::error::Error for Exception {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref().map(|source| source as _)
    }
}

impl IntoApiError for Exception {
    // A code of the library has a fixed status, because the stored status of a gcs exception is the status of the upstream call.
    fn status_code(&self) -> u16 {
        match library_entry(self.code) {
            Some((status, _)) => status,
            None => self
                .status
                .filter(|status| (400..=599).contains(status))
                .unwrap_or(500),
        }
    }

    fn error_code(&self) -> &str {
        self.code
    }

    fn message(&self) -> String {
        if let Some((_, message)) = library_entry(self.code) {
            return message.to_string();
        }
        match self.status {
            Some(400..=499) => self.message.clone(),
            _ => PROJECT_MESSAGE.to_string(),
        }
    }
}

const EXTERNAL_MESSAGE: &str = "external error";

fn external_message(error: &(dyn std::error::Error + 'static), type_name: Option<&str>) -> String {
    if let Some(io_error) = error.downcast_ref::<std::io::Error>() {
        let name = type_name.unwrap_or(std::any::type_name::<std::io::Error>());
        return format!("{EXTERNAL_MESSAGE}: {name} ({:?})", io_error.kind());
    }
    match type_name {
        Some(name) => format!("{EXTERNAL_MESSAGE}: {name}"),
        None => EXTERNAL_MESSAGE.to_string(),
    }
}
const PROJECT_MESSAGE: &str = "internal error";

fn library_entry(code: &str) -> Option<(u16, &'static str)> {
    LIBRARY_CODES
        .iter()
        .find(|(text, _, _)| *text == code)
        .map(|(_, status, message)| (*status, *message))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Location {
    file: &'static str,
    line: u32,
}

#[cfg(test)]
mod tests {
    use super::code;
    use crate::http::ApiError;

    #[test]
    fn display_shows_the_code_and_the_message_only() {
        let root = exception!(code::ENV_INVALID, "port is not a number");
        let middle = exception!(code::ENV_SECRET, "cannot read the secret", caused_by = root);
        let top = exception!(code::ENV_MISSING, "app cannot start", caused_by = middle);
        assert_eq!(top.to_string(), "env_missing: app cannot start");
        let causes: Vec<_> = top.trace().map(|e| e.code()).collect();
        assert_eq!(causes, ["env_missing", "env_secret", "env_invalid"]);
    }

    #[test]
    fn display_and_debug_do_not_show_the_source_text() {
        let leak = "https://host/o?token=ya29.fake-token&sig=abc";
        let external = exception!(
            code::ENV_SECRET,
            "cannot connect",
            source = std::io::Error::other(leak)
        );
        let cause = exception!(code::ENV_INVALID, "inner", caused_by = external.clone());
        let top = exception!(code::ENV_MISSING, "outer", caused_by = cause);
        for error in [&external, &top] {
            for text in [
                error.to_string(),
                format!("{error:?}"),
                format!("{error:#?}"),
            ] {
                assert!(!text.contains("fake-token"), "{text}");
                assert!(!text.contains("token="), "{text}");
                assert!(!text.contains("sig=abc"), "{text}");
            }
        }
        for node in top.trace() {
            assert!(!node.message().contains("fake-token"), "{}", node.message());
        }
        let root = top.trace().last().unwrap();
        assert_eq!(root.code(), code::EXTERNAL);
        assert_eq!(
            root.message(),
            "external error: std::io::error::Error (Other)"
        );
        let debug = format!("{top:?}");
        assert!(
            debug.contains("env_missing") && debug.contains("outer"),
            "{debug}"
        );
    }

    #[test]
    fn equality_compares_code_message_and_status_only() {
        let first = exception!(code::ENV_MISSING, "x");
        let second = exception!(code::ENV_MISSING, "x");
        assert_ne!(first.location(), second.location());
        assert_eq!(first, second);
        let caused = exception!(
            code::ENV_MISSING,
            "x",
            caused_by = exception!(code::ENV_INVALID, "y")
        );
        assert_eq!(first, caused);
        assert_ne!(first, exception!(code::ENV_MISSING, "z"));
        assert_ne!(first, exception!(code::ENV_INVALID, "x"));
        assert_ne!(first, exception!(code::ENV_MISSING, "x", status = 404));
    }

    #[test]
    fn exception_records_the_file_and_the_line_of_the_call() {
        let error = exception!(code::ENV_MISSING, "APP_KEY is required");
        let line = line!() - 1;
        assert_eq!(error.location(), Some((file!(), line)));
        let external = exception!(
            code::ENV_SECRET,
            "x",
            source = std::io::Error::other("refused")
        );
        assert_eq!(external.source_exception().unwrap().location(), None);
    }

    #[test]
    fn display_of_one_exception_has_no_trace() {
        let error = exception!(code::ENV_MISSING, "APP_KEY is required");
        assert_eq!(error.to_string(), "env_missing: APP_KEY is required");
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn with_source_keeps_the_chain_of_an_external_error() {
        let inner = std::io::Error::other("refused");
        let error = exception!(code::ENV_SECRET, "cannot connect", source = inner);
        let causes: Vec<_> = error.trace().map(|e| (e.code(), e.message())).collect();
        assert_eq!(
            causes,
            [
                ("env_secret", "cannot connect"),
                ("external", "external error: std::io::error::Error (Other)")
            ]
        );
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_string(),
            "external: external error: std::io::error::Error (Other)"
        );
    }

    #[test]
    fn with_code_keeps_the_message_status_and_source() {
        let source = exception!(code::ENV_INVALID, "bad");
        let error = exception!(code::ENV_MISSING, "outer", caused_by = source.clone());
        let error = error.with_code("env_secret");
        assert_eq!(error.code(), "env_secret");
        assert_eq!(error.message(), "outer");
        assert_eq!(error.source_exception(), Some(&source));
        assert_eq!(
            exception!(code::ENV_MISSING, "x", status = 404).status(),
            Some(404)
        );
    }

    #[test]
    fn a_custom_code_uses_the_stored_client_or_server_status() {
        use crate::http::IntoApiError;
        assert_eq!(exception!("my_code", "x", status = 404).status_code(), 404);
        assert_eq!(exception!("my_code", "x", status = 503).status_code(), 503);
        for status in [200_u16, 302, 399, 600] {
            assert_eq!(
                exception!("my_code", "x", status = status).status_code(),
                500
            );
        }
        assert_eq!(exception!("my_code", "x").status_code(), 500);
    }

    #[test]
    fn a_library_code_keeps_its_status_over_the_stored_one() {
        use crate::http::IntoApiError;
        let upstream = |code, status| exception!(code, "x", status = status).status_code();
        assert_eq!(upstream(code::GCS_NOT_FOUND, 410), 404);
        assert_eq!(upstream(code::GCS_API, 400), 502);
        assert_eq!(upstream(code::GCS_TRANSPORT, 400), 502);
        assert_eq!(upstream(code::GCS_AUTH, 403), 500);
        assert_eq!(upstream(code::GCS_CONFIG, 400), 500);
        assert_eq!(upstream(code::GCS_LOCAL, 404), 500);
        assert_eq!(upstream(code::AUTH_STORE, 400), 503);
        assert_eq!(upstream(code::ENV_MISSING, 404), 500);
    }

    #[test]
    fn an_exception_given_as_source_keeps_its_code_and_status() {
        let inner = exception!(code::ENV_INVALID, "inner", status = 404);
        let outer = exception!(code::ENV_SECRET, "outer", source = inner.clone());
        let cause = outer.source_exception().unwrap();
        assert_eq!(cause.code(), "env_invalid");
        assert_eq!(cause.status(), Some(404));
        assert_eq!(cause.location(), inner.location());
    }

    #[test]
    fn with_status_replaces_the_status() {
        let error = exception!(code::ENV_MISSING, "x", status = 404);
        assert_eq!(error.clone().with_status(None).status(), None);
        assert_eq!(error.with_status(Some(503)).status(), Some(503));
    }

    #[derive(Debug)]
    struct Outer(std::io::Error);

    impl std::fmt::Display for Outer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "outer https://h/o?token=fake-token")
        }
    }

    impl std::error::Error for Outer {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn external_nodes_have_a_type_name_and_an_io_kind_but_no_source_text() {
        let leak =
            std::io::Error::new(std::io::ErrorKind::NotFound, "https://h/o?token=fake-token");
        let error = exception!(code::ENV_SECRET, "x", source = Outer(leak));
        let messages: Vec<_> = error
            .trace()
            .skip(1)
            .map(|e| e.message().to_string())
            .collect();
        assert!(
            messages[0].starts_with("external error: ") && messages[0].ends_with("::Outer"),
            "{messages:?}"
        );
        assert_eq!(
            messages[1],
            "external error: std::io::error::Error (NotFound)"
        );
        for node in error.trace() {
            assert!(!node.message().contains("fake-token"));
        }
        assert!(!format!("{error:?}").contains("fake-token"));
    }

    #[test]
    fn a_project_code_with_a_client_status_shows_its_message() {
        use crate::http::IntoApiError;
        let client = exception!("my_code", "name is taken", status = 409);
        assert_eq!(IntoApiError::message(&client), "name is taken");
        for hidden in [
            exception!("my_code", "db down", status = 503),
            exception!("my_code", "db down"),
            exception!("my_code", "db down", status = 302),
        ] {
            assert_eq!(IntoApiError::message(&hidden), "internal error");
        }
        let library = exception!(code::GCS_NOT_FOUND, "secret detail", status = 404);
        assert_eq!(IntoApiError::message(&library), "storage object not found");
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "belongs to the library")]
    fn a_project_literal_code_cannot_be_a_library_code() {
        let _ = exception!("gcs_api", "x");
    }

    #[test]
    fn every_library_code_has_a_status_and_a_message() {
        use crate::http::IntoApiError;
        let mut seen = std::collections::HashSet::new();
        for (text, status, message) in super::LIBRARY_CODES {
            assert!(seen.insert(*text), "{text}");
            let error = super::Exception::__new(text, "x", "f", 1).with_status(Some(418));
            assert_eq!(error.status_code(), *status, "{text}");
            assert_eq!(IntoApiError::message(&error), *message, "{text}");
        }
        assert!(seen.contains(code::GCS_LOCAL) && seen.contains(code::EXTERNAL));
    }

    #[test]
    fn api_error_hides_the_message_and_the_trace() {
        let source = exception!(code::ENV_INVALID, "secret detail");
        let error = exception!(code::ENV_MISSING, "secret message", caused_by = source);
        let api = ApiError::from(error);
        assert_eq!(api.status.as_u16(), 500);
        assert_eq!(api.code, "env_missing");
        assert_eq!(api.message, "internal error");
    }
}
