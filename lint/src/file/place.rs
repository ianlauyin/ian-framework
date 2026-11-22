use std::path::{Path, PathBuf};

use crate::files::without_dot;

#[derive(Clone)]
pub(crate) struct Place {
    rel: PathBuf,
}

impl Place {
    pub(crate) fn new(path: &Path, srcs: &[PathBuf]) -> Self {
        let path = without_dot(path);
        let rel = srcs
            .iter()
            .filter_map(|src| {
                let src = without_dot(src);
                let rel = path.strip_prefix(&src).ok().or_else(|| {
                    path.strip_prefix(src.parent()?)
                        .ok()
                        .filter(|rel| rel.starts_with("tests"))
                })?;
                Some((src.components().count(), rel.to_path_buf()))
            })
            .max_by_key(|(depth, _)| *depth)
            .map(|(_, rel)| rel)
            .unwrap_or_else(|| path);
        Self { rel }
    }

    pub(crate) fn in_folder(&self, name: &str) -> bool {
        let mut parts = self.rel.components();
        parts.next().is_some_and(|part| part.as_os_str() == name) && parts.next().is_some()
    }

    pub(crate) fn is_file(&self, name: &str) -> bool {
        self.rel == Path::new(name)
    }

    pub(crate) fn parts(&self) -> Vec<&str> {
        self.rel
            .components()
            .filter_map(|part| part.as_os_str().to_str())
            .collect()
    }
}
