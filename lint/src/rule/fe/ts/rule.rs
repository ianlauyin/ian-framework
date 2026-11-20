use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::edit::Edit;
use crate::rule::fe::ts::source::TsFile;

pub(crate) struct TsFileRule {
    pub(crate) check: fn(&TsFile<'_>) -> Vec<Diagnostic>,
    pub(crate) edit: Option<fn(&TsFile<'_>) -> Vec<Edit>>,
}

pub(crate) struct TsFolderRule {
    pub(crate) check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
    pub(crate) edit: Option<fn(&Path, &FileSet, &[PathBuf]) -> Vec<Edit>>,
}

pub(crate) enum TsRule {
    File(TsFileRule),
    Folder(TsFolderRule),
}

impl TsRule {
    pub(crate) const fn file(
        check: fn(&TsFile<'_>) -> Vec<Diagnostic>,
        edit: Option<fn(&TsFile<'_>) -> Vec<Edit>>,
    ) -> Self {
        Self::File(TsFileRule { check, edit })
    }

    pub(crate) const fn folder(
        check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
        edit: Option<fn(&Path, &FileSet, &[PathBuf]) -> Vec<Edit>>,
    ) -> Self {
        Self::Folder(TsFolderRule { check, edit })
    }
}
