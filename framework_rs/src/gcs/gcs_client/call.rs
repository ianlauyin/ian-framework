use std::time::Duration;

#[derive(Default)]
pub(super) struct Call {
    pub(super) method: reqwest::Method,
    pub(super) url: String,
    pub(super) headers: Vec<(reqwest::header::HeaderName, String)>,
    pub(super) json: Option<serde_json::Value>,
    pub(super) bytes: Option<Vec<u8>>,
    pub(super) extra_status: &'static [u16],
    pub(super) timeout: Option<Duration>,
}
