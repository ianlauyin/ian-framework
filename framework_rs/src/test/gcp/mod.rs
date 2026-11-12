mod credentials;
mod project;
mod response;
mod secret_manager;
#[cfg(feature = "gcs")]
mod storage;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use base64::Engine;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

use crate::gcp::credentials::CachedToken;
use crate::gcp::{http_client_builder, Credentials, SecretManager, TokenSource};

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

pub(crate) struct Reply {
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    body: Vec<u8>,
    declared_length: Option<usize>,
}

impl Reply {
    pub(crate) fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
            declared_length: None,
        }
    }

    #[cfg(feature = "gcs")]
    pub(crate) fn header(mut self, name: &str, value: impl AsRef<[u8]>) -> Self {
        self.headers
            .push((name.to_string(), value.as_ref().to_vec()));
        self
    }

    #[cfg(feature = "gcs")]
    pub(crate) fn declared_length(mut self, length: usize) -> Self {
        self.declared_length = Some(length);
        self
    }

    fn into_bytes(self, addr: SocketAddr) -> Vec<u8> {
        let length = self.declared_length.unwrap_or(self.body.len());
        let mut out = format!(
            "HTTP/1.1 {} X\r\ncontent-type: application/json\r\ncontent-length: {length}\r\nconnection: close\r\n",
            self.status
        )
        .into_bytes();
        for (name, value) in self.headers {
            out.extend_from_slice(format!("{name}: ").as_bytes());
            out.extend_from_slice(&replace_addr(&value, addr));
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(&replace_addr(&self.body, addr));
        out
    }
}

// The server answers one request with each reply, in order. The placeholder `{addr}` in a
// reply header or body is the server address. The handle gives the requests.
pub(crate) async fn serve_script(replies: Vec<Reply>) -> (SocketAddr, JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for reply in replies {
            let (mut socket, _) = listener.accept().await.unwrap();
            requests.push(read_request(&mut socket).await);
            socket.write_all(&reply.into_bytes(addr)).await.unwrap();
        }
        requests
    });
    (addr, server)
}

pub(crate) fn closed_addr() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap()
}

// The token URLs point at `addr`, and the cache holds a valid token with the value `token`.
pub(crate) fn fake_tokens(credentials: Credentials, addr: SocketAddr, token: &str) -> TokenSource {
    TokenSource {
        credentials,
        metadata_token_url: format!("http://{addr}/token"),
        oauth_token_url: format!("http://{addr}/oauth"),
        cached: tokio::sync::Mutex::new(Some(CachedToken {
            value: token.to_string(),
            expires_at: Instant::now() + Duration::from_secs(600),
        })),
    }
}

pub(crate) fn fake_secret_manager(addr: SocketAddr) -> SecretManager {
    SecretManager {
        http: http_client_builder().build().unwrap(),
        api_root: format!("http://{addr}/v1"),
        project: "my-project".to_string(),
        quota_project: Some("my-project".to_string()),
        tokens: fake_tokens(Credentials::Metadata, addr, "fake-token"),
    }
}

fn replace_addr(bytes: &[u8], addr: SocketAddr) -> Vec<u8> {
    let placeholder = b"{addr}";
    let mut out = Vec::new();
    let mut rest = bytes;
    while let Some(at) = rest
        .windows(placeholder.len())
        .position(|window| window == placeholder)
    {
        out.extend_from_slice(&rest[..at]);
        out.extend_from_slice(addr.to_string().as_bytes());
        rest = &rest[at + placeholder.len()..];
    }
    out.extend_from_slice(rest);
    out
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buf = vec![0_u8; 64 * 1024];
    let mut needed = None;
    loop {
        if needed.is_none() {
            if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                needed = Some(end + 4 + length);
            }
        }
        if needed.is_some_and(|needed| request.len() >= needed) {
            return String::from_utf8_lossy(&request).into_owned();
        }
        let read = socket.read(&mut buf).await.unwrap();
        assert!(read > 0, "the client closed the connection early");
        request.extend_from_slice(&buf[..read]);
    }
}
