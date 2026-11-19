use std::fmt;
use std::path::{Path, PathBuf};

pub const IAN_LINT_IGNORE: &str = "ian-lint-ignore";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
    pub rule: &'static str,
    pub problem: String,
    pub fix: String,
}

impl Diagnostic {
    pub fn new(
        path: &Path,
        line: u32,
        column: u32,
        rule: &'static str,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Self {
            path: path.to_path_buf(),
            line,
            column,
            rule,
            problem: problem.into(),
            fix: fix.into(),
        }
    }

    pub fn file(
        path: &Path,
        rule: &'static str,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Self::new(path, 1, 1, rule, problem, fix)
    }

    pub fn at_offset(
        path: &Path,
        source: &str,
        offset: u32,
        rule: &'static str,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        let before = &source[..source.floor_char_boundary(offset as usize)];
        let line = before.matches('\n').count() as u32 + 1;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count() as u32 + 1;
        Self::new(path, line, column, rule, problem, fix)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{} ian-lint/{}: {}\n    fix: {}",
            self.path.display(),
            self.line,
            self.column,
            self.rule,
            self.problem,
            self.fix
        )
    }
}

pub fn suppress(source: &str, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    if !source.contains(IAN_LINT_IGNORE) {
        return diagnostics;
    }
    let lines: Vec<&str> = source.lines().collect();
    let ignores = |line: u32, rule: &str| {
        lines.get(line as usize - 1).is_some_and(|text| {
            text.split(IAN_LINT_IGNORE).skip(1).any(|rest| {
                rest.trim_start()
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
                    .next()
                    == Some(rule)
            })
        })
    };
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            !ignores(diagnostic.line, diagnostic.rule)
                && !(diagnostic.line > 1 && ignores(diagnostic.line - 1, diagnostic.rule))
        })
        .collect()
}
