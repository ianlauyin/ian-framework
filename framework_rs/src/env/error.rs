use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvErrorKind {
    Missing,
    NotUnicode,
    Invalid,
}

// The error never holds the value of the variable, because the value can be a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvError {
    kind: EnvErrorKind,
    name: String,
}

impl EnvError {
    pub fn invalid(name: &str) -> Self {
        Self::new(EnvErrorKind::Invalid, name)
    }

    pub fn kind(&self) -> EnvErrorKind {
        self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn new(kind: EnvErrorKind, name: &str) -> Self {
        Self {
            kind,
            name: name.to_string(),
        }
    }
}

impl fmt::Display for EnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            EnvErrorKind::Missing => write!(f, "{} is required", self.name),
            EnvErrorKind::NotUnicode => write!(f, "{} is not valid unicode", self.name),
            EnvErrorKind::Invalid => write!(f, "{} is not valid", self.name),
        }
    }
}

impl std::error::Error for EnvError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_reports_the_name() {
        let error = EnvError::invalid("APP_KEY");
        assert_eq!(error.kind(), EnvErrorKind::Invalid);
        assert_eq!(error.name(), "APP_KEY");
    }

    #[test]
    fn display_gives_one_message_for_each_kind() {
        assert_eq!(
            EnvError::new(EnvErrorKind::Missing, "APP_KEY").to_string(),
            "APP_KEY is required"
        );
        assert_eq!(
            EnvError::new(EnvErrorKind::NotUnicode, "APP_KEY").to_string(),
            "APP_KEY is not valid unicode"
        );
        assert_eq!(
            EnvError::invalid("APP_KEY").to_string(),
            "APP_KEY is not valid"
        );
    }

    #[test]
    fn is_a_std_error() {
        let error: Box<dyn std::error::Error> = Box::new(EnvError::invalid("APP_KEY"));
        assert!(error.source().is_none());
    }
}
