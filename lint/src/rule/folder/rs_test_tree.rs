// The `tests` folder has no `mod.rs`.
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

use crate::config::Side;
use crate::file::rs::source;
use crate::files::{FileSet, file_name, without_dot};
use crate::rule::diagnostic::Diagnostic;
use crate::rule::folder::FolderRule;
use crate::rule::report::RuleReport;

const RULE: &str = "test-tree";

pub(crate) const FOLDER_RULE: FolderRule = FolderRule::new(Side::Be, check);

pub fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    if !srcs.iter().any(|src| without_dot(src) == without_dot(path)) {
        return Vec::new();
    }
    let mut diagnostics = check_tree(path, &path.join("tests"), false, files);
    if let Some(parent) = path.parent() {
        diagnostics.extend(check_tree(path, &parent.join("tests"), true, files));
    }
    diagnostics
}

fn check_tree(src: &Path, test_root: &Path, sibling: bool, files: &FileSet) -> Vec<Diagnostic> {
    if !files.has_dir(test_root) && (sibling || !files.has_file(&test_root.join("mod.rs"))) {
        return Vec::new();
    }
    let mut diagnostics = Vec::new();
    for dir in files.dirs() {
        if !dir.starts_with(test_root) || (sibling && dir == test_root) {
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
            "The `tests` folder has no `mod.rs`.",
            "Add a `mod.rs` that declares the child modules.",
        ));
    }
    let mut pairs = Vec::new();
    for file in files.files() {
        let Ok(relative) = file.strip_prefix(test_root) else {
            continue;
        };
        if !file_name(file).ends_with(".rs") {
            continue;
        }
        if is_root(relative, sibling) {
            pairs.push((file, crate_root(src, files)));
            continue;
        }
        let source = source_file(src, relative, sibling, files);
        if files.has_file(&source) {
            pairs.push((file, source));
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
    for (file, source) in pairs {
        if files.has_file(&source) {
            diagnostics.extend(cfg_mismatch(file, &source));
        }
    }
    diagnostics
}

fn is_root(relative: &Path, sibling: bool) -> bool {
    if sibling {
        relative.components().count() == 1
    } else {
        relative == Path::new("mod.rs")
    }
}

fn source_file(src: &Path, relative: &Path, sibling: bool, files: &FileSet) -> PathBuf {
    let source = src.join(relative);
    if !sibling || files.has_file(&source) || file_name(relative) != "mod.rs" {
        return source;
    }
    let Some(folder) = relative.parent() else {
        return source;
    };
    let mut file = src.join(folder).into_os_string();
    file.push(".rs");
    let file = PathBuf::from(file);
    if files.has_file(&file) { file } else { source }
}

fn crate_root(path: &Path, files: &FileSet) -> PathBuf {
    let lib = path.join("lib.rs");
    if files.has_file(&lib) {
        lib
    } else {
        path.join("main.rs")
    }
}

fn cfg_mismatch(test_file: &Path, source_file: &Path) -> Vec<Diagnostic> {
    let Ok(test_source) = fs::read_to_string(test_file) else {
        return Vec::new();
    };
    let Ok(source_source) = fs::read_to_string(source_file) else {
        return Vec::new();
    };
    let Some(source_file_ast) = source::parse(&source_source) else {
        return Vec::new();
    };
    let Some(test_ast) = source::parse(&test_source) else {
        return Vec::new();
    };
    let source_mods = mod_cfgs(&source_file_ast.items);
    let mut report = RuleReport::new(test_file, &test_source, RULE);
    for item in &test_ast.items {
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
        report.at_span(
            module.ident.span(),
            format!("The cfg on `{name}` does not match the source module."),
            "Use the same `#[cfg(...)]` as the source module.",
        );
    }
    report.finish()
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
