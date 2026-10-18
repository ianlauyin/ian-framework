#[cfg(feature = "logging")]
pub mod logging;

#[cfg(feature = "gcs")]
pub mod gcs;

#[cfg(test)]
mod tests {
    #[test]
    fn logging_is_a_default_cargo_feature() {
        let manifest = include_str!("../Cargo.toml");
        let features = manifest.split("[features]").nth(1).expect("features table");
        let features = features.split("\n[").next().unwrap_or(features);
        let default_line = features
            .lines()
            .find(|line| line.trim_start().starts_with("default"))
            .expect("default feature");
        assert!(
            default_line.contains("\"logging\""),
            "default features must include logging"
        );
        assert!(!default_line.contains("gcs"), "gcs stays optional");
    }
}
