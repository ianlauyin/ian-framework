use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

pub(crate) const FILE_NAME: &str = "lint.config.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ruleset {
    Framework,
    Project,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    All,
    Fe,
    Rs,
}

impl Side {
    pub(crate) fn checks_fe(self) -> bool {
        matches!(self, Self::All | Self::Fe)
    }

    pub(crate) fn checks_rs(self) -> bool {
        matches!(self, Self::All | Self::Rs)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) ruleset: Ruleset,
    pub(crate) fe: Vec<PathBuf>,
    pub(crate) be: Vec<PathBuf>,
}

pub(crate) fn load(root: &Path) -> Result<Config, String> {
    let path = root.join(FILE_NAME);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let file = match serde_json::from_str::<File>(&text) {
        Ok(file) => file,
        Err(error) => return Err(format!("{FILE_NAME}: {error}")),
    };
    let ruleset = match file.ruleset.as_str() {
        "framework" => Ruleset::Framework,
        "project" => Ruleset::Project,
        _ => {
            return Err(format!("{FILE_NAME}: `{}` is not a ruleset", file.ruleset));
        }
    };
    Ok(Config {
        ruleset,
        fe: src_paths(root, &file.fe.paths)?,
        be: src_paths(root, &file.be.paths)?,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    ruleset: String,
    fe: Paths,
    be: Paths,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Paths {
    paths: Vec<String>,
}

fn src_paths(root: &Path, paths: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut srcs = Vec::new();
    for path in paths {
        srcs.push(src_path(root, path)?);
    }
    Ok(srcs)
}

fn src_path(root: &Path, path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(path);
    if path.is_empty() || relative.is_absolute() {
        return Err(format!("{FILE_NAME}: `{path}` is not a src path"));
    }
    let mut out = root.to_path_buf();
    for part in relative.components() {
        match part {
            Component::Normal(name) => out.push(name),
            Component::CurDir => {}
            _ => return Err(format!("{FILE_NAME}: `{path}` is not a src path")),
        }
    }
    Ok(out)
}
