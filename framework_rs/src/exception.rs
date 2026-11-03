use std::fmt;

use crate::http::IntoApiError;

#[macro_export]
macro_rules! exception {
    ($message:expr $(, code = $code:expr)? $(, severity = $severity:expr)? $(, source = $source:expr)? $(,)?) => {{
        let exception = $crate::exception::Exception::__new($message, file!(), line!());
        $( let exception = exception.with_code($code); )?
        $( let exception = exception.with_severity($severity); )?
        $( let exception = exception.__with_source($source); )?
        exception
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warn,
    Error,
}

#[derive(Clone)]
pub struct Exception {
    severity: Severity,
    code: Option<&'static str>,
    message: String,
    location: Option<Location>,
    source: Option<Box<Exception>>,
}

impl Exception {
    #[doc(hidden)]
    pub fn __new(message: impl Into<String>, file: &'static str, line: u32) -> Self {
        Self {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            location: Some(Location { file, line }),
            source: None,
        }
    }

    #[doc(hidden)]
    pub fn __with_source<E: std::error::Error + 'static>(self, source: E) -> Self {
        let source: &(dyn std::error::Error + 'static) = &source;
        let cause = match source.downcast_ref::<Exception>() {
            Some(exception) => exception.clone(),
            None => Self::from_error(source, Some(std::any::type_name::<E>())),
        };
        Self {
            source: Some(Box::new(cause)),
            ..self
        }
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn code(&self) -> Option<&'static str> {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    // The file and the line of the `exception!` call. An external exception has none.
    pub fn location(&self) -> Option<(&'static str, u32)> {
        self.location.map(|location| (location.file, location.line))
    }

    pub fn source_exception(&self) -> Option<&Exception> {
        self.source.as_deref()
    }

    pub fn with_code(self, code: &'static str) -> Self {
        Self {
            code: Some(code),
            ..self
        }
    }

    pub fn with_severity(self, severity: Severity) -> Self {
        Self { severity, ..self }
    }

    // The trace starts at this exception and ends at the root source.
    pub fn trace(&self) -> impl Iterator<Item = &Exception> {
        std::iter::successors(Some(self), |exception| exception.source_exception())
    }

    // The message has only a type name and an `io::ErrorKind`, never the text of the source.
    fn from_error(error: &(dyn std::error::Error + 'static), type_name: Option<&str>) -> Self {
        Self {
            severity: Severity::Error,
            code: None,
            message: external_message(error, type_name),
            location: None,
            source: error
                .source()
                .map(|source| Box::new(Self::from_error(source, None))),
        }
    }
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Warn => "WARN",
            Severity::Error => "ERROR",
        }
    }
}

impl fmt::Debug for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exception")
            .field("severity", &self.severity)
            .field("code", &self.code)
            .field("message", &self.message)
            .field("location", &self.location())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl PartialEq for Exception {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code && self.message == other.message
    }
}

impl Eq for Exception {}

impl std::error::Error for Exception {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref().map(|source| source as _)
    }
}

impl From<&Exception> for String {
    fn from(exception: &Exception) -> Self {
        let mut lines = Vec::new();
        for (index, node) in exception.trace().enumerate() {
            let mut line = format!("{index}: {} ", node.severity.as_str());
            if let Some(code) = node.code {
                line.push_str(&format!("[{code}] "));
            }
            line.push_str(&node.message);
            if let Some(location) = node.location {
                line.push_str(&format!(" at {}:{}", location.file, location.line));
            }
            lines.push(line);
        }
        lines.join("\n")
    }
}

impl From<Exception> for String {
    fn from(exception: Exception) -> Self {
        Self::from(&exception)
    }
}

impl IntoApiError for Exception {
    fn status_code(&self) -> u16 {
        500
    }

    fn error_code(&self) -> &str {
        self.code.unwrap_or(INTERNAL_CODE)
    }

    fn message(&self) -> String {
        INTERNAL_MESSAGE.to_string()
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

const INTERNAL_CODE: &str = "INTERNAL_ERROR";
const INTERNAL_MESSAGE: &str = "internal error";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Location {
    file: &'static str,
    line: u32,
}

#[cfg(test)]
mod tests {
    use crate::http::ApiError;

    #[test]
    fn display_shows_the_current_message_only() {
        let root = exception!("port is not a number", code = "ENV_INVALID");
        let middle = exception!("cannot read the secret", code = "ENV_SECRET", source = root);
        let top = exception!("app cannot start", code = "ENV_MISSING", source = middle);
        assert_eq!(top.to_string(), "app cannot start");
        let causes: Vec<_> = top.trace().map(|e| e.code()).collect();
        assert_eq!(
            causes,
            [Some("ENV_MISSING"), Some("ENV_SECRET"), Some("ENV_INVALID")]
        );
    }

    #[test]
    fn display_and_debug_do_not_show_the_source_text() {
        let leak = "https://host/o?token=ya29.fake-token&sig=abc";
        let external = exception!(
            "cannot connect",
            code = "ENV_SECRET",
            source = std::io::Error::other(leak)
        );
        let cause = exception!("inner", code = "ENV_INVALID", source = external.clone());
        let top = exception!("outer", code = "ENV_MISSING", source = cause);
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
        assert_eq!(root.code(), None);
        assert_eq!(
            root.message(),
            "external error: std::io::error::Error (Other)"
        );
        let debug = format!("{top:?}");
        assert!(
            debug.contains("ENV_MISSING") && debug.contains("outer"),
            "{debug}"
        );
    }

    #[test]
    fn equality_compares_code_and_message_only() {
        let first = exception!("x", code = "ENV_MISSING");
        let second = exception!("x", code = "ENV_MISSING");
        assert_ne!(first.location(), second.location());
        assert_eq!(first, second);
        let caused = exception!(
            "x",
            code = "ENV_MISSING",
            source = exception!("y", code = "ENV_INVALID")
        );
        assert_eq!(first, caused);
        assert_ne!(first, exception!("z", code = "ENV_MISSING"));
        assert_ne!(first, exception!("x", code = "ENV_INVALID"));
    }

    #[test]
    fn exception_records_the_file_and_the_line_of_the_call() {
        let error = exception!("APP_KEY is required", code = "ENV_MISSING");
        let line = line!() - 1;
        assert_eq!(error.location(), Some((file!(), line)));
        let external = exception!(
            "x",
            code = "ENV_SECRET",
            source = std::io::Error::other("refused")
        );
        assert_eq!(external.source_exception().unwrap().location(), None);
    }

    #[test]
    fn display_of_one_exception_has_no_trace() {
        let error = exception!("APP_KEY is required", code = "ENV_MISSING");
        assert_eq!(error.to_string(), "APP_KEY is required");
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn with_source_keeps_the_chain_of_an_external_error() {
        let inner = std::io::Error::other("refused");
        let error = exception!("cannot connect", code = "ENV_SECRET", source = inner);
        let causes: Vec<_> = error.trace().map(|e| (e.code(), e.message())).collect();
        assert_eq!(
            causes,
            [
                (Some("ENV_SECRET"), "cannot connect"),
                (None, "external error: std::io::error::Error (Other)")
            ]
        );
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_string(),
            "external error: std::io::error::Error (Other)"
        );
    }

    #[test]
    fn with_code_keeps_the_message_and_source() {
        let source = exception!("bad", code = "ENV_INVALID");
        let error = exception!("outer", code = "ENV_MISSING", source = source.clone());
        let error = error.with_code("ENV_SECRET");
        assert_eq!(error.code(), Some("ENV_SECRET"));
        assert_eq!(error.message(), "outer");
        assert_eq!(error.source_exception(), Some(&source));
    }

    #[test]
    fn every_exception_is_a_500_with_an_internal_message() {
        use crate::http::IntoApiError;
        for error in [
            exception!("db down", code = "MY_CODE"),
            exception!("secret detail", code = "GCS_NOT_FOUND"),
        ] {
            assert_eq!(error.status_code(), 500);
            assert_eq!(IntoApiError::message(&error), "internal error");
        }
    }

    #[test]
    fn an_exception_given_as_source_keeps_its_code() {
        let inner = exception!("inner", code = "ENV_INVALID");
        let outer = exception!("outer", code = "ENV_SECRET", source = inner.clone());
        let cause = outer.source_exception().unwrap();
        assert_eq!(cause.code(), Some("ENV_INVALID"));
        assert_eq!(cause.location(), inner.location());
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
        let error = exception!("x", code = "ENV_SECRET", source = Outer(leak));
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
    fn log_text_has_every_message_with_its_code_and_location() {
        let root = exception!("port is not a number", code = "ENV_INVALID");
        let line = line!() - 1;
        let top = exception!("app cannot start", code = "ENV_MISSING", source = root);
        let top_line = line!() - 1;
        let external = exception!(
            "cannot connect",
            code = "ENV_SECRET",
            source = std::io::Error::other("refused")
        );
        let file = file!();
        assert_eq!(
            String::from(&top),
            format!(
                "0: ERROR [ENV_MISSING] app cannot start at {file}:{top_line}\n1: ERROR [ENV_INVALID] port is not a number at {file}:{line}"
            )
        );
        assert!(String::from(&external)
            .ends_with("1: ERROR external error: std::io::error::Error (Other)"));
        assert!(!String::from(external).contains("refused"));
    }

    #[test]
    fn a_warn_exception_without_a_code_shows_its_severity_only() {
        let warn = exception!("slow", severity = super::Severity::Warn);
        assert_eq!(warn.code(), None);
        assert_eq!(warn.severity(), super::Severity::Warn);
        assert!(String::from(&warn).starts_with("0: WARN slow at "));
        assert_eq!(ApiError::from(warn).code, "INTERNAL_ERROR");
    }

    #[test]
    fn api_error_hides_the_message_and_the_trace() {
        let source = exception!("secret detail", code = "ENV_INVALID");
        let error = exception!("secret message", code = "ENV_MISSING", source = source);
        let api = ApiError::from(error);
        assert_eq!(api.status.as_u16(), 500);
        assert_eq!(api.code, "ENV_MISSING");
        assert_eq!(api.message, "internal error");
    }
}
