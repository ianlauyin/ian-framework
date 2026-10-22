use std::time::Duration;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use super::super::encode::{encode_object_path, encode_seg};
use super::super::{GcsError, GcsErrorKind};
use super::{validate_object_name, GcsBackend};

const HOST: &str = "storage.googleapis.com";
const MAX_SIGNED_URL_SECONDS: u64 = 604_800;

impl GcsBackend {
    pub async fn signed_get_url(
        &self,
        name: &str,
        expires_in: Option<Duration>,
    ) -> Result<String, GcsError> {
        validate_object_name(name)?;
        let seconds = signed_lifetime_seconds(expires_in)?;
        let parts = SignedParts::v4_get(
            self.bucket(),
            name,
            self.signer_email(),
            Utc::now(),
            seconds,
        );
        let signature = self.sign_blob(parts.string_to_sign.as_bytes()).await?;
        Ok(format!(
            "{}&X-Goog-Signature={}",
            parts.url_prefix,
            hex::encode(signature)
        ))
    }
}

struct SignedParts {
    string_to_sign: String,
    url_prefix: String,
}

impl SignedParts {
    fn v4_get(
        bucket_name: &str,
        name: &str,
        email: &str,
        now: DateTime<Utc>,
        expires_secs: u64,
    ) -> Self {
        let request_ts = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date = now.format("%Y%m%d").to_string();
        let scope = format!("{date}/auto/storage/goog4_request");
        let credential = format!("{email}/{scope}");
        let canonical_uri = format!("/{}/{}", encode_seg(bucket_name), encode_object_path(name));
        let mut pairs = [
            ("X-Goog-Algorithm", "GOOG4-RSA-SHA256".to_string()),
            ("X-Goog-Credential", credential),
            ("X-Goog-Date", request_ts.clone()),
            ("X-Goog-Expires", expires_secs.to_string()),
            ("X-Goog-SignedHeaders", "host".to_string()),
        ];
        pairs.sort_by(|left, right| left.0.cmp(right.0));
        let mut query = String::new();
        for (index, (key, value)) in pairs.iter().enumerate() {
            if index > 0 {
                query.push('&');
            }
            query.push_str(&encode_seg(key));
            query.push('=');
            query.push_str(&encode_seg(value));
        }
        let canonical_request =
            format!("GET\n{canonical_uri}\n{query}\nhost:{HOST}\n\nhost\nUNSIGNED-PAYLOAD");
        let hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
        let string_to_sign = format!("GOOG4-RSA-SHA256\n{request_ts}\n{scope}\n{hash}");
        let url_prefix = format!("https://{HOST}{canonical_uri}?{query}");
        Self {
            string_to_sign,
            url_prefix,
        }
    }
}

fn signed_lifetime_seconds(expires_in: Option<Duration>) -> Result<u64, GcsError> {
    let Some(expires_in) = expires_in else {
        return Err(GcsError::new(
            GcsErrorKind::Auth,
            "signed URL lifetime is missing",
        ));
    };
    let seconds = expires_in.as_secs();
    if seconds == 0 {
        return Err(GcsError::new(
            GcsErrorKind::Auth,
            "signed URL lifetime must be at least 1 second",
        ));
    }
    if seconds > MAX_SIGNED_URL_SECONDS {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "signed URL lifetime must be at most 604800 seconds",
        ));
    }
    Ok(seconds)
}

#[cfg(test)]
mod tests {
    use super::super::super::cred::Auth;
    use super::*;

    #[test]
    fn v4_get_matches_the_canonical_request() {
        let now = DateTime::parse_from_rfc3339("2018-10-26T21:19:42Z")
            .unwrap()
            .with_timezone(&Utc);
        let parts = SignedParts::v4_get(
            "example-bucket",
            "photos/cat.jpeg",
            "signer@example-project.iam.gserviceaccount.com",
            now,
            3600,
        );
        let canonical = "\
GET
/example-bucket/photos/cat.jpeg
X-Goog-Algorithm=GOOG4-RSA-SHA256&X-Goog-Credential=signer%40example-project.iam.gserviceaccount.com%2F20181026%2Fauto%2Fstorage%2Fgoog4_request&X-Goog-Date=20181026T211942Z&X-Goog-Expires=3600&X-Goog-SignedHeaders=host
host:storage.googleapis.com

host
UNSIGNED-PAYLOAD";
        let hash = hex::encode(Sha256::digest(canonical.as_bytes()));
        assert_eq!(
            hash,
            "efdb769ae1f6dc9f5fc558b88db91be5e7dc5abec2dbb8a135ea7ea85bd8fdd4"
        );
        let expected_sts = format!(
            "GOOG4-RSA-SHA256\n20181026T211942Z\n20181026/auto/storage/goog4_request\n{hash}"
        );
        assert_eq!(parts.string_to_sign, expected_sts);
        assert_eq!(
            parts.url_prefix,
            "https://storage.googleapis.com/example-bucket/photos/cat.jpeg?X-Goog-Algorithm=GOOG4-RSA-SHA256&X-Goog-Credential=signer%40example-project.iam.gserviceaccount.com%2F20181026%2Fauto%2Fstorage%2Fgoog4_request&X-Goog-Date=20181026T211942Z&X-Goog-Expires=3600&X-Goog-SignedHeaders=host"
        );
    }

    #[test]
    fn v4_encodes_spaces_in_the_object_path() {
        let now = DateTime::parse_from_rfc3339("2018-10-26T21:19:42Z")
            .unwrap()
            .with_timezone(&Utc);
        let parts = SignedParts::v4_get(
            "b",
            "my cat.jpeg",
            "s@example.iam.gserviceaccount.com",
            now,
            60,
        );
        assert!(parts.url_prefix.contains("/b/my%20cat.jpeg?"));
        assert!(parts
            .string_to_sign
            .contains("20181026/auto/storage/goog4_request"));
    }

    #[tokio::test]
    async fn signed_url_rejects_a_lifetime_over_seven_days() {
        let backend = GcsBackend::new(
            "my-bucket",
            Auth::User {
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                refresh_token: "refresh".to_string(),
                signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
            },
        )
        .await
        .unwrap();
        let err = backend
            .signed_get_url("a", Some(Duration::from_secs(604_801)))
            .await
            .unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }

    #[tokio::test]
    async fn signed_url_rejects_a_missing_or_zero_lifetime() {
        let backend = GcsBackend::new(
            "my-bucket",
            Auth::User {
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                refresh_token: "refresh".to_string(),
                signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
            },
        )
        .await
        .unwrap();
        for lifetime in [None, Some(Duration::ZERO), Some(Duration::from_millis(500))] {
            let err = backend.signed_get_url("a", lifetime).await.unwrap_err();
            assert_eq!(err.kind(), GcsErrorKind::Auth);
        }
    }

    #[test]
    fn signed_lifetime_never_returns_zero() {
        assert!(signed_lifetime_seconds(None).is_err());
        assert!(signed_lifetime_seconds(Some(Duration::ZERO)).is_err());
        assert!(signed_lifetime_seconds(Some(Duration::from_millis(999))).is_err());
        assert_eq!(
            signed_lifetime_seconds(Some(Duration::from_secs(1))).unwrap(),
            1
        );
    }
}
