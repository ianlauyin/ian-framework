use super::{LogEvent, LogValue};

pub(crate) const MASK: &str = "***";

// access_log records the query and the JSON body, so a credential field must not reach a log.
pub const DEFAULT_MASK_PATTERNS: [&str; 7] = [
    r"(?i)password",
    r"(?i)passwd",
    r"(?i)secret",
    r"(?i)token",
    r"(?i)authorization",
    r"(?i)api[_-]?key",
    r"(?i)cookie",
];

// `pairs` match a `name=value` pair whose name holds a match of a pattern, so the value is masked too.
pub(crate) struct Masks {
    plains: Vec<regex::Regex>,
    pairs: Vec<regex::Regex>,
}

impl Masks {
    pub(crate) fn new(patterns: &[String]) -> Self {
        let plains = patterns
            .iter()
            .map(|pattern| compile(pattern, pattern))
            .collect();
        let pairs = patterns
            .iter()
            .map(|pattern| compile(pattern, &format!(r"[\w.\-]*(?:{pattern})[\w.\-]*=[^&\s;]*")))
            .collect();
        Self { plains, pairs }
    }

    pub(crate) fn mask_event(&self, event: &mut LogEvent) {
        for (key, value) in event.context.iter_mut() {
            self.mask_entry(key, value);
        }
        for (key, value) in event.extra.iter_mut() {
            self.mask_entry(key, value);
        }
    }

    fn mask_entry(&self, key: &str, value: &mut LogValue) {
        if self.plains.iter().any(|plain| plain.is_match(key)) {
            *value = LogValue::String(MASK.to_string());
        } else if let LogValue::String(text) = value {
            for regex in self.pairs.iter().chain(&self.plains) {
                if let std::borrow::Cow::Owned(masked) = regex.replace_all(text, MASK) {
                    *text = masked;
                }
            }
        }
    }
}

fn compile(pattern: &str, source: &str) -> regex::Regex {
    regex::Regex::new(source)
        .unwrap_or_else(|err| panic!("mask pattern `{pattern}` is not a valid regex: {err}"))
}
