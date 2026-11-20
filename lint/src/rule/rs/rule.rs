use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::edit::Edit;
use crate::rule::rs::source::RsFile;

pub(crate) struct RsFileRule {
    pub(crate) check: fn(&RsFile<'_>) -> Vec<Diagnostic>,
    pub(crate) edit: Option<fn(&RsFile<'_>) -> Vec<Edit>>,
}

pub(crate) struct RsFolderRule {
    pub(crate) check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
    pub(crate) edit: Option<fn(&Path, &FileSet, &[PathBuf]) -> Vec<Edit>>,
}

pub(crate) enum RsRule {
    File(RsFileRule),
    Folder(RsFolderRule),
}

impl RsRule {
    pub(crate) const fn file(
        check: fn(&RsFile<'_>) -> Vec<Diagnostic>,
        edit: Option<fn(&RsFile<'_>) -> Vec<Edit>>,
    ) -> Self {
        Self::File(RsFileRule { check, edit })
    }

    pub(crate) const fn folder(
        check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
        edit: Option<fn(&Path, &FileSet, &[PathBuf]) -> Vec<Edit>>,
    ) -> Self {
        Self::Folder(RsFolderRule { check, edit })
    }
}
