use crate::gcs::gcs_backend::GcsBackend;
use crate::gcs::GcsError;

impl GcsBackend {
    pub async fn get_bytes(&self, name: &str) -> Result<Vec<u8>, GcsError> {
        self.open_object(name).await?.read_to_vec().await
    }
}
