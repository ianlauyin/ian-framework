use std::ffi::OsString;
use std::path::PathBuf;

use crate::config::Side;

pub const USAGE: &str = "Usage: ian-lint [fe|be] [root]";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Check { root: PathBuf, side: Side },
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let mut root = None;
    let mut side = None;
    for arg in args {
        if arg == "-h" || arg == "--help" {
            return Ok(Command::Help);
        }
        if let Some(found) = side_arg(&arg) {
            if side.replace(found).is_some() {
                return Err("The side is set two times.".to_owned());
            }
            continue;
        }
        if arg.to_string_lossy().starts_with('-') {
            return Err(format!(
                "`{}` is not an argument.\n{USAGE}",
                arg.to_string_lossy()
            ));
        }
        if root.replace(PathBuf::from(arg)).is_some() {
            return Err("The root is set two times.".to_owned());
        }
    }
    Ok(Command::Check {
        root: root.unwrap_or_else(|| PathBuf::from(".")),
        side: side.unwrap_or(Side::All),
    })
}

fn side_arg(arg: &OsString) -> Option<Side> {
    if arg == "fe" {
        Some(Side::Fe)
    } else if arg == "be" {
        Some(Side::Be)
    } else {
        None
    }
}
