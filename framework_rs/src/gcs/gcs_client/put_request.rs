use crate::gcs::{PutBody, PutOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PutRequest {
    pub name: String,
    pub body: PutBody,
    pub options: PutOptions,
}
