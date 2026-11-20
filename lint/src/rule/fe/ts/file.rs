use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::Parser;
use oxc_span::SourceType;

pub(crate) struct TsFile<'a> {
    pub(crate) path: &'a Path,
    pub(crate) source: &'a str,
    pub(crate) program: Option<&'a Program<'a>>,
}

impl<'a> TsFile<'a> {
    pub(crate) fn new(path: &'a Path, source: &'a str, program: Option<&'a Program<'a>>) -> Self {
        Self {
            path,
            source,
            program,
        }
    }
}

pub(crate) fn parse<'a>(
    allocator: &'a Allocator,
    path: &Path,
    source: &'a str,
) -> Option<Program<'a>> {
    let source_type = SourceType::from_path(path).ok()?;
    Some(Parser::new(allocator, source, source_type).parse().program)
}

#[cfg(test)]
pub(crate) fn with_source<T>(path: &Path, source: &str, f: impl FnOnce(&TsFile) -> T) -> T {
    let allocator = Allocator::default();
    let program = parse(&allocator, path, source);
    f(&TsFile::new(path, source, program.as_ref()))
}
