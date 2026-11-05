use std::fmt;

#[macro_export]
macro_rules! exception {
    ($message:expr, code = $code:expr $(, severity = $severity:expr)? $(, source = $source:expr)? $(,)?) => {{
        let exception = $crate::exception::Exception::__new($message, $code, file!(), line!());
        $( let exception = exception.with_severity($severity); )?
        $( let exception = exception.__with_source($source); )?
        exception
    }};
}

#[derive(Debug, PartialEq, Eq)]
pub enum Severity {
    Warn,
    Error,
}

pub struct Exception {
    severity: Severity,
    code: &'static str,
    message: String,
    location: Location,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Exception {
    #[doc(hidden)]
    pub fn __new(
        message: impl Into<String>,
        code: &'static str,
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            location: Location { file, line },
            source: None,
        }
    }

    #[doc(hidden)]
    pub fn __with_source<E>(self, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            source: Some(Box::new(source)),
            ..self
        }
    }

    pub fn severity(&self) -> &Severity {
        &self.severity
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    // The file and the line of the `exception!` call.
    pub fn location(&self) -> (&'static str, u32) {
        (self.location.file, self.location.line)
    }

    pub fn source_exception(&self) -> Option<&Exception> {
        self.source.as_deref()?.downcast_ref::<Exception>()
    }

    pub fn with_code(self, code: &'static str) -> Self {
        Self { code, ..self }
    }

    pub fn with_severity(self, severity: Severity) -> Self {
        Self { severity, ..self }
    }

    // The trace starts at this exception and follows each source that is an Exception.
    pub fn trace(&self) -> impl Iterator<Item = &Exception> {
        std::iter::successors(Some(self), |exception| exception.source_exception())
    }
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
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

impl std::error::Error for Exception {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

impl From<&Exception> for String {
    fn from(exception: &Exception) -> Self {
        let mut lines = Vec::new();
        for (index, node) in exception.trace().enumerate() {
            lines.push(format!(
                "{index}: {} [{}] {} at {}:{}",
                node.severity.as_str(),
                node.code,
                node.message,
                node.location.file,
                node.location.line
            ));
        }
        lines.join("\n")
    }
}

impl From<Exception> for String {
    fn from(exception: Exception) -> Self {
        Self::from(&exception)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Location {
    file: &'static str,
    line: u32,
}
