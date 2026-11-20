use std::path::Path;

use proc_macro2::Span;

use crate::rule::diagnostic::Diagnostic;

pub(crate) struct RuleReport<'a> {
    path: &'a Path,
    source: &'a str,
    rule: &'static str,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> RuleReport<'a> {
    pub(crate) fn new(path: &'a Path, source: &'a str, rule: &'static str) -> Self {
        Self {
            path,
            source,
            rule,
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn at_offset(
        &mut self,
        offset: u32,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic::at_offset(
            self.path,
            self.source,
            offset,
            self.rule,
            problem,
            fix,
        ));
    }

    pub(crate) fn at_span(
        &mut self,
        span: Span,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) {
        self.at_offset(span.byte_range().start as u32, problem, fix);
    }

    pub(crate) fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
