use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use ignore::WalkBuilder;

#[derive(Default)]
pub struct FileSet {
    files: BTreeSet<PathBuf>,
    dirs: BTreeSet<PathBuf>,
}

impl FileSet {
    pub fn walk(root: &Path) -> (Self, Vec<String>) {
        let mut errors = Vec::new();
        let mut paths = Vec::new();
        for entry in WalkBuilder::new(root).require_git(false).build() {
            match entry {
                Ok(entry) if entry.file_type().is_some_and(|kind| kind.is_file()) => {
                    let path = entry.into_path();
                    paths.push(
                        path.strip_prefix("./")
                            .map(Path::to_path_buf)
                            .unwrap_or(path),
                    );
                }
                Ok(_) => {}
                Err(error) => errors.push(error.to_string()),
            }
        }
        (Self::from_paths(paths), errors)
    }

    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut set = Self::default();
        for path in paths {
            for dir in path.ancestors().skip(1) {
                if !set.dirs.insert(dir.to_path_buf()) {
                    break;
                }
            }
            set.files.insert(path);
        }
        set
    }

    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.files.iter().map(PathBuf::as_path)
    }

    pub fn has_file(&self, path: &Path) -> bool {
        self.files.contains(path)
    }

    pub fn has_dir(&self, path: &Path) -> bool {
        self.dirs.contains(path)
    }

    pub fn module_name<'a>(&self, path: &'a Path) -> Option<&'a str> {
        path.ancestors()
            .skip(1)
            .find(|dir| self.has_file(&dir.join("module.tsx")))?
            .file_name()?
            .to_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Component,
    Module,
}

pub fn scope(path: &Path, src: &Path) -> Option<Scope> {
    let path = without_dot(path);
    let src = without_dot(src);
    let rest = path.strip_prefix(&src).ok()?;
    match rest.components().next()?.as_os_str().to_str() {
        Some("component") => Some(Scope::Component),
        Some("module") => Some(Scope::Module),
        _ => None,
    }
}

pub fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
}

pub fn is_component_css(path: &Path, srcs: &[PathBuf]) -> bool {
    file_name(path) == "index.css" && srcs.iter().any(|src| scope(path, src).is_some())
}

pub fn is_ts_name(name: &str) -> bool {
    name.ends_with(".ts") || name.ends_with(".tsx")
}

pub fn is_ts_source(path: &Path, srcs: &[PathBuf]) -> bool {
    let name = file_name(path);
    under_one(path, srcs) && is_ts_name(name) && !name.ends_with(".d.ts")
}

pub fn is_frontend_source(path: &Path, srcs: &[PathBuf]) -> bool {
    is_ts_source(path, srcs) || (under_one(path, srcs) && file_name(path).ends_with(".css"))
}

pub fn is_rust_source(path: &Path, srcs: &[PathBuf]) -> bool {
    under_one(path, srcs) && file_name(path).ends_with(".rs")
}

fn under_one(path: &Path, srcs: &[PathBuf]) -> bool {
    let path = without_dot(path);
    srcs.iter().any(|src| path.starts_with(without_dot(src)))
}

fn without_dot(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        if !matches!(part, Component::CurDir) {
            out.push(part);
        }
    }
    out
}
