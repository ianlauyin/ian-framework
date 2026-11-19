// `{name}` is missing from the index.
// Re-export `{name}` from the index.
// Why a lint rule: The root index re-exports each feature.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;
use crate::rule::finder::Finder;

const RULE: &str = "root-reexport";

pub fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    if !srcs.iter().any(|src| src == path) {
        return Vec::new();
    }
    let children = child_modules(path, files);
    let child_names: Vec<&str> = children.iter().map(String::as_str).collect();
    let mut readable = Vec::new();
    for index in index_paths(path, files) {
        let Ok(source) = fs::read_to_string(&index) else {
            continue;
        };
        readable.push((index, source));
    }
    match readable.as_slice() {
        [] => Vec::new(),
        [(index, source)] => check_source(index, source, &child_names),
        indexes => {
            let mut exported = BTreeSet::new();
            for (index, source) in indexes {
                exported.extend(exported_names(index, source));
            }
            let (index, source) = &indexes[0];
            report_missing(index, source, &children, &exported)
        }
    }
}

pub(crate) fn check_source(path: &Path, source: &str, children: &[&str]) -> Vec<Diagnostic> {
    let exported = exported_names(path, source);
    let children: Vec<String> = children.iter().map(|name| (*name).to_string()).collect();
    report_missing(path, source, &children, &exported)
}

fn report_missing(
    path: &Path,
    source: &str,
    children: &[String],
    exported: &BTreeSet<String>,
) -> Vec<Diagnostic> {
    let mut missing: Vec<&str> = children
        .iter()
        .map(String::as_str)
        .filter(|name| !exported.contains(*name))
        .collect();
    missing.sort_unstable();
    let mut finder = Finder::new(path, source);
    for name in missing {
        finder.report(
            0,
            RULE,
            format!("`./{name}` is missing from the index."),
            format!("Re-export `./{name}` from the index."),
        );
    }
    finder.finish()
}

fn index_paths(dir: &Path, files: &FileSet) -> Vec<PathBuf> {
    ["index.ts", "index.tsx"]
        .into_iter()
        .map(|name| dir.join(name))
        .filter(|path| files.has_file(path))
        .collect()
}

fn child_modules(dir: &Path, files: &FileSet) -> Vec<String> {
    let mut names = BTreeSet::new();
    for file in files.files() {
        let Ok(relative) = file.strip_prefix(dir) else {
            continue;
        };
        let mut parts = relative.components();
        let Some(first) = parts.next() else {
            continue;
        };
        let Some(name) = first.as_os_str().to_str() else {
            continue;
        };
        if parts.next().is_some() {
            if is_ts_file(file) {
                names.insert(name.to_string());
            }
            continue;
        }
        if name == "index.ts" || name == "index.tsx" {
            continue;
        }
        if name.ends_with(".test.ts") || name.ends_with(".test.tsx") {
            continue;
        }
        if !is_ts_file(file) {
            continue;
        }
        let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        names.insert(stem.to_string());
    }
    names.into_iter().collect()
}

fn is_ts_file(path: &Path) -> bool {
    crate::files::is_ts_name(crate::files::file_name(path))
}

fn exported_names(path: &Path, source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Ok(source_type) = SourceType::from_path(path) else {
        return names;
    };
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    for statement in &parsed.program.body {
        let specifier = match statement {
            Statement::ExportAllDeclaration(declaration) => declaration.source.value.as_str(),
            Statement::ExportFromDeclaration(declaration) => declaration.source.value.as_str(),
            _ => None,
        };
        let Some(specifier) = specifier else {
            continue;
        };
        let Some(name) = module_name(specifier) else {
            continue;
        };
        names.insert(name);
    }
    names
}

fn module_name(specifier: &str) -> Option<String> {
    let rest = specifier.strip_prefix("./")?;
    if rest.is_empty() {
        return None;
    }
    let rest = rest
        .strip_suffix(".tsx")
        .or_else(|| rest.strip_suffix(".ts"))
        .unwrap_or(rest);
    let rest = rest.strip_suffix("/index").unwrap_or(rest);
    if rest.is_empty() || rest.contains('/') || rest.contains('\\') {
        return None;
    }
    Some(rest.to_string())
}
