mod credentials;
mod error;
mod http_client;
mod secret_manager;

pub(crate) use credentials::{metadata_get, CachedToken, Credentials};
pub(crate) use error::{api_detail, truncate, GcpError, GcpErrorKind};
pub(crate) use http_client::http_client_builder;
pub(crate) use secret_manager::SecretManager;

#[cfg(feature = "gcs")]
pub(crate) use credentials::{metadata_token, UserCredentials};

#[cfg(test)]
pub(crate) mod tests {
    use std::net::SocketAddr;
    use std::time::{Duration, Instant};

    use base64::Engine;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::task::JoinHandle;

    use crate::gcp::{http_client_builder, CachedToken, Credentials, SecretManager};

    pub(crate) fn ok_body(value: &[u8]) -> Vec<u8> {
        let data = base64::engine::general_purpose::STANDARD.encode(value);
        format!(r#"{{"name":"projects/my-project/secrets/KEY/versions/1","payload":{{"data":"{data}","dataCrc32c":"1"}}}}"#)
            .into_bytes()
    }

    // The server answers one request. The handle gives the request head in lower case.
    pub(crate) async fn serve(status: u16, body: Vec<u8>) -> (SocketAddr, JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut head = Vec::new();
            let mut buf = [0_u8; 1024];
            while !head.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = socket.read(&mut buf).await.unwrap();
                assert!(read > 0);
                head.extend_from_slice(&buf[..read]);
            }
            let reply = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
            String::from_utf8(head).unwrap().to_ascii_lowercase()
        });
        (addr, server)
    }

    // The tests must not call Google Cloud, so the shared Secret Manager is set to "no credentials".
    pub(crate) fn no_secret_manager() {
        let _ = crate::gcp::secret_manager::SECRET_MANAGER.set(None);
    }

    pub(crate) fn closed_addr() -> SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    }

    pub(crate) fn fake_secret_manager(addr: SocketAddr) -> SecretManager {
        SecretManager {
            http: http_client_builder().build().unwrap(),
            api_root: format!("http://{addr}/v1"),
            project: "my-project".to_string(),
            quota_project: Some("my-project".to_string()),
            credentials: Credentials::Metadata,
            token: tokio::sync::Mutex::new(CachedToken {
                value: "fake-token".to_string(),
                expires_at: Instant::now() + Duration::from_secs(3600),
            }),
        }
    }
}
