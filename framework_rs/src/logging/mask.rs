use super::{LogEvent, LogValue};
use crate::exception;
use crate::exception::Exception;

pub const MASK_PATTERN_INVALID: &str = "MASK_PATTERN_INVALID";

pub const MASK: &str = "***";

pub const DEFAULT_MASK_PATTERNS: [&str; 7] = [
    r"(?i)password",
    r"(?i)passwd",
    r"(?i)secret",
    r"(?i)token",
    r"(?i)authorization",
    r"(?i)api[_-]?key",
    r"(?i)cookie",
];

pub struct Masks {
    names: Option<regex::Regex>,
    pairs: Vec<regex::Regex>,
}

impl Masks {
    pub fn new(patterns: &[String]) -> Result<Self, Exception> {
        if patterns.is_empty() {
            return Ok(Self {
                names: None,
                pairs: Vec::new(),
            });
        }
        for pattern in patterns {
            compile(pattern, pattern)?;
        }
        let any = patterns
            .iter()
            .map(|pattern| format!("(?:{pattern})"))
            .collect::<Vec<_>>()
            .join("|");
        let pairs = [json_member, name_value, name_line]
            .iter()
            .map(|form| compile(&any, &form(&any)))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            names: Some(compile(&any, &any)?),
            pairs,
        })
    }

    pub fn mask_event(&self, event: &mut LogEvent) {
        for (key, value) in event.context.iter_mut() {
            self.mask_entry(key, value);
        }
        for (key, value) in event.extra.iter_mut() {
            self.mask_entry(key, value);
        }
        if let Some(text) = &mut event.error_message {
            self.mask_text(text);
        }
    }

    fn mask_entry(&self, key: &str, value: &mut LogValue) {
        if self.names.as_ref().is_some_and(|names| names.is_match(key)) {
            *value = LogValue::String(MASK.to_string());
        } else if let LogValue::String(text) = value {
            self.mask_text(text);
        }
    }

    fn mask_text(&self, text: &mut String) {
        for pair in &self.pairs {
            if let std::borrow::Cow::Owned(masked) = pair.replace_all(text, MASK) {
                *text = masked;
            }
        }
    }
}

fn json_member(pattern: &str) -> String {
    format!(r#""[^"]*(?:{pattern})[^"]*"\s*:\s*(?:"(?:[^"\\]|\\.)*"|[^\s,}}\]]+)"#)
}

fn name_value(pattern: &str) -> String {
    format!(r"[\w.\-]*(?:{pattern})[\w.\-]*=[^&\s;]*")
}

fn name_line(pattern: &str) -> String {
    format!(r#"[\w.\-]*(?:{pattern})[\w.\-]*:[ \t]+[^\r\n"]*"#)
}

fn compile(pattern: &str, source: &str) -> Result<regex::Regex, Exception> {
    regex::Regex::new(source).map_err(|err| {
        exception!(
            code = MASK_PATTERN_INVALID,
            format!("mask pattern `{pattern}` is not a valid regex"),
            source = err,
        )
    })
}
