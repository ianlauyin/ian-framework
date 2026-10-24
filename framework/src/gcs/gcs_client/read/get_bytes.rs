use crate::gcs::gcs_client::GcsClient;
use crate::gcs::GcsError;

impl GcsClient {
    pub async fn get_bytes(&self, name: &str) -> Result<Vec<u8>, GcsError> {
        self.open_object(name).await?.read_to_vec().await
    }
}
