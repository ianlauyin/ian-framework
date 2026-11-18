use std::fmt;
use std::path::{Path, PathBuf};

pub const CODE_STYLE_IGNORE: &str = "code-style-lint-ignore";

pub const RUST_IGNORE: &str = "rs-lint-ignore";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tool {
    CodeStyle,
    RsStyle,
    IanLint,
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CodeStyle => "code-style",
            Self::RsStyle => "rs-style",
            Self::IanLint => "ian-lint",
        })
    }
}

impl Tool {
    fn ignore_marker(self) -> Option<&'static str> {
        match self {
            Self::CodeStyle => Some(CODE_STYLE_IGNORE),
            Self::RsStyle => Some(RUST_IGNORE),
            Self::IanLint => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
    pub tool: Tool,
    pub rule: &'static str,
    pub problem: String,
    pub fix: String,
}

impl Diagnostic {
    pub fn new(
        path: &Path,
        line: u32,
        column: u32,
        tool: Tool,
        rule: &'static str,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Self {
            path: path.to_path_buf(),
            line,
            column,
            tool,
            rule,
            problem: problem.into(),
            fix: fix.into(),
        }
    }

    pub fn file(
        path: &Path,
        tool: Tool,
        rule: &'static str,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Self::new(path, 1, 1, tool, rule, problem, fix)
    }

    pub fn at_offset(
        path: &Path,
        source: &str,
        offset: u32,
        tool: Tool,
        rule: &'static str,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        let before = &source[..(offset as usize).min(source.len())];
        let line = before.matches('\n').count() as u32 + 1;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count() as u32 + 1;
        Self::new(path, line, column, tool, rule, problem, fix)
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

pub fn suppress(source: &str, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    if !source.contains(CODE_STYLE_IGNORE) && !source.contains(RUST_IGNORE) {
        return diagnostics;
    }
    let lines: Vec<&str> = source.lines().collect();
    let ignores = |line: u32, marker: &str, rule: &str| {
        lines.get(line as usize - 1).is_some_and(|text| {
            text.split(marker).skip(1).any(|rest| {
                rest.trim_start()
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
                    .next()
                    == Some(rule)
            })
        })
    };
    diagnostics
        .into_iter()
        .filter(|diagnostic| match diagnostic.tool.ignore_marker() {
            Some(marker) => {
                !ignores(diagnostic.line, marker, diagnostic.rule)
                    && !(diagnostic.line > 1
                        && ignores(diagnostic.line - 1, marker, diagnostic.rule))
            }
            None => true,
        })
        .collect()
}
