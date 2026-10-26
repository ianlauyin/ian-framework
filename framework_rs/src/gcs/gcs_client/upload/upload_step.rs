use crate::gcs::{GcsError, GcsErrorKind, ObjectMeta};

pub(super) enum UploadStep {
    Done(ObjectMeta),
    Continue { next: u64 },
}

impl UploadStep {
    pub(super) async fn from_response(
        response: reqwest::Response,
        fallback_next: u64,
    ) -> Result<Self, GcsError> {
        if response.status().is_success() {
            return Ok(Self::Done(ObjectMeta::from_response(response).await?));
        }
        let header = response
            .headers()
            .get(reqwest::header::RANGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let _ = response.bytes().await;
        let next = match header {
            Some(value) => next_offset_from_range(&value)?,
            None => fallback_next,
        };
        Ok(Self::Continue { next })
    }

    pub(super) fn expect_done(self) -> Result<ObjectMeta, GcsError> {
        match self {
            Self::Done(meta) => Ok(meta),
            Self::Continue { .. } => Err(GcsError::new(
                GcsErrorKind::Api,
                "finished upload did not return object metadata",
            )),
        }
    }
}

fn next_offset_from_range(header: &str) -> Result<u64, GcsError> {
    let header = header.trim();
    let rest = header
        .strip_prefix("bytes=")
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    let rest = rest.split(',').next().unwrap_or(rest).trim();
    let (start, end) = rest
        .split_once('-')
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    let start: u64 = start
        .trim()
        .parse()
        .map_err(|_| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    let end: u64 = end
        .trim()
        .parse()
        .map_err(|_| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    if end < start {
        return Err(GcsError::new(
            GcsErrorKind::Api,
            "resumable Range header is invalid",
        ));
    }
    end.checked_add(1)
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_range_is_the_end_plus_one() {
        assert_eq!(next_offset_from_range("bytes=0-42").unwrap(), 43);
        assert_eq!(next_offset_from_range("bytes=0-0").unwrap(), 1);
        assert!(next_offset_from_range("bytes=*").is_err());
        assert!(next_offset_from_range("0-1").is_err());
    }
}
