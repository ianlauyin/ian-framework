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
                path.strip_prefix(&src)
                    .ok()
                    .map(|rel| (src.components().count(), rel.to_path_buf()))
            })
            .max_by_key(|(depth, _)| *depth)
            .map(|(_, rel)| rel)
            .unwrap_or_else(|| path);
        Self { rel }
    }

    pub(crate) fn in_test(&self) -> bool {
        let mut parts = self.rel.components();
        parts.next().is_some_and(|part| part.as_os_str() == "test") && parts.next().is_some()
    }

    pub(crate) fn in_gcp(&self) -> bool {
        let mut parts = self.rel.components();
        parts.next().is_some_and(|part| part.as_os_str() == "gcp") && parts.next().is_some()
    }

    pub(crate) fn is_file(&self, name: &str) -> bool {
        self.rel == Path::new(name)
    }
}
