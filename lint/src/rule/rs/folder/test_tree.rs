// The test folder has no `mod.rs`.
// Add a `mod.rs` that declares the child modules.
// The test file has no source file `{path}`.
// Give the test the same path as the source file.
// The cfg on `{name}` does not match the source module.
// Use the same `#[cfg(...)]` as the source module.
// Why a lint rule: The test tree matches the crate.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use syn::{Attribute, Item, Meta};

use crate::diagnostic::Diagnostic;
use crate::files::{FileSet, file_name, without_dot};
use crate::rule::finder::Finder;

const RULE: &str = "test-tree";

pub fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    if !srcs.iter().any(|src| without_dot(src) == without_dot(path)) {
        return Vec::new();
    }
    let test_root = path.join("test");
    if !files.has_dir(&test_root) && !files.has_file(&test_root.join("mod.rs")) {
        return Vec::new();
    }
    let mut diagnostics = Vec::new();
    for dir in files.dirs() {
        if !dir.starts_with(&test_root) {
            continue;
        }
        if files.has_file(&dir.join("mod.rs")) {
            continue;
        }
        diagnostics.push(Diagnostic::new(
            dir,
            1,
            1,
            RULE,
            "The test folder has no `mod.rs`.",
            "Add a `mod.rs` that declares the child modules.",
        ));
    }
    let mut rust_files = Vec::new();
    for file in files.files() {
        if file.strip_prefix(&test_root).is_err() || !file_name(file).ends_with(".rs") {
            continue;
        }
        rust_files.push(file);
    }
    for file in &rust_files {
        let Ok(relative) = file.strip_prefix(&test_root) else {
            continue;
        };
        if relative == Path::new("mod.rs") {
            continue;
        }
        let source = path.join(relative);
        if files.has_file(&source) {
            continue;
        }
        diagnostics.push(Diagnostic::new(
            file,
            1,
            1,
            RULE,
            format!("The test file has no source file `{}`.", source.display()),
            "Give the test the same path as the source file.",
        ));
    }
    for file in rust_files {
        let Ok(relative) = file.strip_prefix(&test_root) else {
            continue;
        };
        let source = if relative == Path::new("mod.rs") {
            path.join("lib.rs")
        } else {
            path.join(relative)
        };
        if !files.has_file(&source) {
            continue;
        }
        diagnostics.extend(cfg_mismatch(file, &source));
    }
    diagnostics
}

fn cfg_mismatch(test_file: &Path, source_file: &Path) -> Vec<Diagnostic> {
    let Ok(test_source) = fs::read_to_string(test_file) else {
        return Vec::new();
    };
    let Ok(source_source) = fs::read_to_string(source_file) else {
        return Vec::new();
    };
    let Ok(test_file_ast) = syn::parse_file(&test_source) else {
        return Vec::new();
    };
    let Ok(source_file_ast) = syn::parse_file(&source_source) else {
        return Vec::new();
    };
    let source_mods = mod_cfgs(&source_file_ast.items);
    let mut finder = Finder::new(test_file, &test_source);
    for item in &test_file_ast.items {
        let Item::Mod(module) = item else {
            continue;
        };
        let name = module.ident.to_string();
        let Some(expected) = source_mods.get(&name) else {
            continue;
        };
        if &cfg_keys(&module.attrs) == expected {
            continue;
        }
        finder.report(
            module.ident.span().byte_range().start as u32,
            RULE,
            format!("The cfg on `{name}` does not match the source module."),
            "Use the same `#[cfg(...)]` as the source module.",
        );
    }
    finder.finish()
}

fn mod_cfgs(items: &[Item]) -> BTreeMap<String, Vec<String>> {
    let mut mods = BTreeMap::new();
    for item in items {
        let Item::Mod(module) = item else {
            continue;
        };
        mods.entry(module.ident.to_string())
            .or_insert_with(|| cfg_keys(&module.attrs));
    }
    mods
}

fn cfg_keys(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("cfg") {
                return None;
            }
            match &attr.meta {
                Meta::List(list) => Some(list.tokens.to_string()),
                Meta::Path(_) => Some(String::from("path")),
                Meta::NameValue(_) => Some(String::from("value")),
            }
        })
        .collect()
}
