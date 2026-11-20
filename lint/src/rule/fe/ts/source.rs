use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::rule::diagnostic::line_column;

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

pub(crate) fn parse_valid<'a>(
    allocator: &'a Allocator,
    path: &Path,
    source: &'a str,
) -> Result<Program<'a>, String> {
    let source_type = SourceType::from_path(path)
        .map_err(|error| format!("{}: The file is not TypeScript: {error}", path.display()))?;
    let parsed = Parser::new(allocator, source, source_type).parse();
    let Some(error) = parsed.diagnostics.errors().next() else {
        return Ok(parsed.program);
    };
    let offset = error.labels.first().map_or(0, |label| label.offset());
    let (line, column) = line_column(source, offset);
    Err(format!(
        "{}:{line}:{column}: The file does not parse: {error}",
        path.display()
    ))
}
