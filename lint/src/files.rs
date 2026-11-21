use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use ignore::WalkBuilder;

#[derive(Default)]
pub struct FileSet {
    files: BTreeSet<PathBuf>,
    dirs: BTreeSet<PathBuf>,
}

impl FileSet {
    pub(crate) fn walk(root: &Path) -> (Self, Vec<String>) {
        let mut errors = Vec::new();
        let mut paths = Vec::new();
        let mut dir_paths = Vec::new();
        for entry in WalkBuilder::new(root).require_git(false).build() {
            match entry {
                Ok(entry) => {
                    let is_file = entry.file_type().is_some_and(|kind| kind.is_file());
                    let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
                    if !is_file && !is_dir {
                        continue;
                    }
                    let path = entry.into_path();
                    let path = path
                        .strip_prefix("./")
                        .map(Path::to_path_buf)
                        .unwrap_or(path);
                    if is_file {
                        paths.push(path);
                    } else {
                        dir_paths.push(path);
                    }
                }
                Err(error) => errors.push(error.to_string()),
            }
        }
        let mut set = Self::from_paths(paths);
        for dir in dir_paths {
            set.dirs.insert(dir);
        }
        (set, errors)
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

    pub(crate) fn files(&self) -> impl Iterator<Item = &Path> {
        self.files.iter().map(PathBuf::as_path)
    }

    pub(crate) fn dirs(&self) -> impl Iterator<Item = &Path> {
        self.dirs.iter().map(PathBuf::as_path)
    }

    pub(crate) fn has_file(&self, path: &Path) -> bool {
        self.files.contains(path)
    }

    pub(crate) fn has_dir(&self, path: &Path) -> bool {
        self.dirs.contains(path)
    }
}

pub(crate) fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
}

pub(crate) fn in_src(path: &Path, srcs: &[PathBuf]) -> bool {
    let path = without_dot(path);
    srcs.iter().any(|src| path.starts_with(without_dot(src)))
}

pub(crate) fn is_ts_name(name: &str) -> bool {
    (name.ends_with(".ts") || name.ends_with(".tsx")) && !name.ends_with(".d.ts")
}

pub(crate) fn is_style_file(path: &Path, srcs: &[PathBuf]) -> bool {
    in_src(path, srcs) && file_name(path).ends_with(".css")
}

pub(crate) fn is_frontend_source(path: &Path, srcs: &[PathBuf]) -> bool {
    (in_src(path, srcs) && is_ts_name(file_name(path))) || is_style_file(path, srcs)
}

pub(crate) fn is_rust_source(path: &Path, srcs: &[PathBuf]) -> bool {
    in_src(path, srcs) && file_name(path).ends_with(".rs")
}

pub(crate) fn without_dot(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        if !matches!(part, Component::CurDir) {
            out.push(part);
        }
    }
    out
}
