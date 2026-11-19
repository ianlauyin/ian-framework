use std::path::Path;

use crate::diagnostic::Diagnostic;

pub struct Finder<'a> {
    path: &'a Path,
    source: &'a str,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Finder<'a> {
    pub fn new(path: &'a Path, source: &'a str) -> Self {
        Self {
            path,
            source,
            diagnostics: Vec::new(),
        }
    }

    pub fn report(
        &mut self,
        offset: u32,
        rule: &'static str,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic::at_offset(
            self.path,
            self.source,
            offset,
            rule,
            problem,
            fix,
        ));
    }

    pub fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
