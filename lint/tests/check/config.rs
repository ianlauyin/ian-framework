use std::fs;

use ian_lint::config::{Config, Ruleset, load};

use crate::project::{CONFIG, project};

fn load_text(name: &str, text: &str) -> Result<Config, String> {
    let root = project(name, &[("lint.config.json", text.as_bytes())]);
    let config = load(&root);
    fs::remove_dir_all(root).expect("test folder is removed");
    config
}

#[test]
fn a_config_with_two_src_folders_passes() {
    let root = project("config-ok", &[("lint.config.json", CONFIG.as_bytes())]);
    let config = load(&root);
    fs::remove_dir_all(&root).expect("test folder is removed");
    assert_eq!(
        config,
        Ok(Config {
            ruleset: Ruleset::Project,
            fe: vec![root.join("web/src")],
            be: vec![root.join("api/src")],
        })
    );
    let framework = CONFIG
        .replace("project", "framework")
        .replace("web/src", "./web/src");
    let config = load_text("config-framework", &framework).expect("config is loaded");
    assert_eq!(config.ruleset, Ruleset::Framework);
    let empty = "{\"ruleset\":\"project\",\"fe\":{\"paths\":[]},\"be\":{\"paths\":[]}}";
    assert!(load_text("config-empty", empty).is_ok());
}

#[test]
fn a_config_with_no_file_fails() {
    let root = project("config-missing", &[]);
    let config = load(&root);
    fs::remove_dir_all(root).expect("test folder is removed");
    assert!(config.is_err());
}

#[test]
fn a_ruleset_that_is_not_known_fails() {
    assert_eq!(
        load_text("config-ruleset", &CONFIG.replace("project", "other")),
        Err("lint.config.json: `other` is not a ruleset".to_owned())
    );
}

#[test]
fn a_key_that_is_not_known_fails() {
    assert!(load_text("config-key", &CONFIG.replace("\"be\"", "\"rs\"")).is_err());
}

#[test]
fn a_src_path_that_is_not_a_folder_fails() {
    assert_eq!(
        load_text("config-folder", &CONFIG.replace("api/src", "api/scr")),
        Err("lint.config.json: `api/scr` is not a folder".to_owned())
    );
}

#[test]
fn a_src_path_outside_the_root_fails() {
    for path in ["../api/src", "/api/src", ""] {
        assert_eq!(
            load_text("config-outside", &CONFIG.replace("api/src", path)),
            Err(format!("lint.config.json: `{path}` is not a src path"))
        );
    }
}
