use serde::{Deserialize, Serialize};

use crate::logging::LogEvent;

#[derive(Debug, Serialize, Deserialize)]
pub struct LogRequest {
    pub events: Vec<LogEvent>,
}
