use std::path::Path;

use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use proc_macro2::Span;

use crate::diagnostic::Diagnostic;

pub struct Finder<'a> {
    path: &'a Path,
    source: &'a str,
    rule: &'static str,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Finder<'a> {
    pub fn new(path: &'a Path, source: &'a str, rule: &'static str) -> Self {
        Self {
            path,
            source,
            rule,
            diagnostics: Vec::new(),
        }
    }

    pub fn report(&mut self, offset: u32, problem: impl Into<String>, fix: impl Into<String>) {
        self.diagnostics.push(Diagnostic::at_offset(
            self.path,
            self.source,
            offset,
            self.rule,
            problem,
            fix,
        ));
    }

    pub fn at_span(&mut self, span: Span, problem: impl Into<String>, fix: impl Into<String>) {
        self.report(span.byte_range().start as u32, problem, fix);
    }

    pub fn rust_source(
        path: &'a Path,
        source: &'a str,
        rule: &'static str,
        f: impl FnOnce(&syn::File, &mut Finder<'a>),
    ) -> Vec<Diagnostic> {
        let Ok(file) = syn::parse_file(source) else {
            return Vec::new();
        };
        let mut finder = Finder::new(path, source, rule);
        f(&file, &mut finder);
        finder.finish()
    }

    pub fn typescript_source(
        path: &'a Path,
        source: &'a str,
        rule: &'static str,
        f: impl for<'b> FnOnce(&'b oxc_ast::ast::Program<'b>, &mut Finder<'a>),
    ) -> Vec<Diagnostic> {
        let Ok(source_type) = SourceType::from_path(path) else {
            return Vec::new();
        };
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, source_type).parse();
        let mut finder = Finder::new(path, source, rule);
        f(&parsed.program, &mut finder);
        finder.finish()
    }

    pub fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
