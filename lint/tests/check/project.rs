use std::path::PathBuf;
use std::{env, fs, process};

pub(crate) const CONFIG: &str =
    "{\"ruleset\":\"project\",\"fe\":{\"paths\":[\"web/src\"]},\"be\":{\"paths\":[\"api/src\"]}}";

pub(crate) fn project(name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let root = env::temp_dir().join(format!("ian-lint-check-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    for folder in ["web/src", "api/src"] {
        fs::create_dir_all(root.join(folder)).expect("folder is created");
    }
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("path has a folder")).expect("folder is created");
        fs::write(path, text).expect("file is written");
    }
    root
}
