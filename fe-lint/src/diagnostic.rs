use std::fmt;
use std::path::{Path, PathBuf};

/// A comment with this text and a rule id stops that rule on the same line and on the next line.
/// The form is `/* code-style-lint-ignore rule-id: the reason */`.
pub const IGNORE_MARKER: &str = "code-style-lint-ignore";

/// The linter that reports a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tool {
    /// The rules of this program, in `fe-lint/src/rule/`.
    CodeStyle,
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CodeStyle => "code-style",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Diagnostic {
    pub path: PathBuf,
    /// 1-based.
    pub line: u32,
    /// 1-based.
    pub column: u32,
    pub tool: Tool,
    pub rule: &'static str,
    /// What is wrong.
    pub problem: String,
    /// What to change to correct it.
    pub fix: String,
}

impl Diagnostic {
    /// A diagnostic of a rule of this program.
    pub fn new(path: &Path, line: u32, column: u32, rule: &'static str, problem: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            path: path.to_path_buf(),
            line,
            column,
            tool: Tool::CodeStyle,
            rule,
            problem: problem.into(),
            fix: fix.into(),
        }
    }

    /// For an error that applies to the file, not to a line.
    pub fn file(path: &Path, rule: &'static str, problem: impl Into<String>, fix: impl Into<String>) -> Self {
        Self::new(path, 1, 1, rule, problem, fix)
    }

    /// `offset` is a byte offset in `source`.
    pub fn at_offset(path: &Path, source: &str, offset: u32, rule: &'static str, problem: impl Into<String>, fix: impl Into<String>) -> Self {
        let before = &source[..(offset as usize).min(source.len())];
        let line = before.matches('\n').count() as u32 + 1;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count() as u32 + 1;
        Self::new(path, line, column, rule, problem, fix)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{} {}/{}: {}\n    fix: {}",
            self.path.display(),
            self.line,
            self.column,
            self.tool,
            self.rule,
            self.problem,
            self.fix
        )
    }
}

/// Removes the diagnostics that an ignore comment in `source` stops.
pub fn suppress(source: &str, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    if !source.contains(IGNORE_MARKER) {
        return diagnostics;
    }
    let lines: Vec<&str> = source.lines().collect();
    let ignores = |line: u32, rule: &str| {
        lines.get(line as usize - 1).is_some_and(|text| {
            text.split(IGNORE_MARKER)
                .skip(1)
                .any(|rest| rest.trim_start().split(|c: char| !c.is_ascii_alphanumeric() && c != '-').next() == Some(rule))
        })
    };
    diagnostics
        .into_iter()
        .filter(|diagnostic| !ignores(diagnostic.line, diagnostic.rule) && !(diagnostic.line > 1 && ignores(diagnostic.line - 1, diagnostic.rule)))
        .collect()
}
