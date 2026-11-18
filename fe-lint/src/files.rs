//! The file tree, and the meaning of a path in the project structure.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

/// All files below the root, and all their folders.
#[derive(Default)]
pub struct FileSet {
    files: BTreeSet<PathBuf>,
    dirs: BTreeSet<PathBuf>,
}

impl FileSet {
    /// The walk obeys `.gitignore`, also when the root is not in a git repository.
    /// The second value is the errors of the walk, for example a folder that it cannot read: the files in it are not in the set.
    #[allow(clippy::filetype_is_file, reason = "A symbolic link is not a file of the project, and the walk does not follow it.")]
    pub fn walk(root: &Path) -> (Self, Vec<String>) {
        let mut errors = Vec::new();
        let mut paths = Vec::new();
        for entry in WalkBuilder::new(root).require_git(false).build() {
            match entry {
                Ok(entry) if entry.file_type().is_some_and(|kind| kind.is_file()) => {
                    let path = entry.into_path();
                    paths.push(path.strip_prefix("./").map(Path::to_path_buf).unwrap_or(path));
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

    /// The module that owns `path`: the name of the nearest folder that has a `module.tsx`.
    pub fn module_name<'a>(&self, path: &'a Path) -> Option<&'a str> {
        path.ancestors().skip(1).find(|dir| self.has_file(&dir.join("module.tsx")))?.file_name()?.to_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Below `src/component`.
    Component,
    /// Below `src/module`.
    Module,
}

pub fn scope(path: &Path) -> Option<Scope> {
    let parts: Vec<_> = path.components().map(std::path::Component::as_os_str).collect();
    parts.iter().zip(parts.iter().skip(1)).find_map(|(first, second)| match (first.to_str(), second.to_str()) {
        (Some("src"), Some("component")) => Some(Scope::Component),
        (Some("src"), Some("module")) => Some(Scope::Module),
        _ => None,
    })
}

pub fn in_src(path: &Path) -> bool {
    path.components().any(|part| part.as_os_str() == "src")
}

pub fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|name| name.to_str()).unwrap_or("")
}

/// The `index.css` of a component: `**/src/component/**/index.css` or `**/src/module/**/index.css`.
pub fn is_component_css(path: &Path) -> bool {
    file_name(path) == "index.css" && scope(path).is_some()
}

pub fn is_ts_name(name: &str) -> bool {
    name.ends_with(".ts") || name.ends_with(".tsx")
}

/// A `.ts` or `.tsx` source file below a `src` folder. A `.d.ts` file is not one.
pub fn is_ts_source(path: &Path) -> bool {
    let name = file_name(path);
    in_src(path) && is_ts_name(name) && !name.ends_with(".d.ts")
}
