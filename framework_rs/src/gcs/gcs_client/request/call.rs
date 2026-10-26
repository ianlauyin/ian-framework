use std::time::Duration;

#[derive(Default)]
pub(crate) struct Call {
    pub(crate) method: reqwest::Method,
    pub(crate) url: String,
    pub(crate) headers: Vec<(reqwest::header::HeaderName, String)>,
    pub(crate) json: Option<serde_json::Value>,
    pub(crate) bytes: Option<Vec<u8>>,
    pub(crate) extra_status: &'static [u16],
    pub(crate) timeout: Option<Duration>,
}
