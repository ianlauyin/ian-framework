use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Diagnostic {
    pub(crate) path: PathBuf,
    pub(crate) line: u32,
    pub(crate) column: u32,
    pub(crate) rule: &'static str,
    pub(crate) problem: String,
    pub(crate) fix: String,
}

impl Diagnostic {
    pub(crate) fn new(
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

    pub(crate) fn at_offset(
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
