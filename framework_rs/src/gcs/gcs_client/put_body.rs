use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PutBody {
    Bytes(Vec<u8>),
    File(PathBuf),
}
