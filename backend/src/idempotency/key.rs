use crate::utils::ApiError;
use actix_web::HttpRequest;

#[derive(Debug)]
pub struct IdempotencyKey(String);

impl TryFrom<String> for IdempotencyKey {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.is_empty() {
            anyhow::bail!("The idempotency key cannot be empty");
        }
        let max_length = 50;
        if s.len() >= max_length {
            anyhow::bail!("The idempotency key must be shorter than {max_length} characters");
        }
        Ok(Self(s))
    }
}

impl From<IdempotencyKey> for String {
    fn from(k: IdempotencyKey) -> Self {
        k.0
    }
}

impl AsRef<str> for IdempotencyKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

pub fn idempotency_key_from_request(request: &HttpRequest) -> Result<IdempotencyKey, ApiError> {
    let header_value = request
        .headers()
        .get("Idempotency-Key")
        .ok_or_else(|| ApiError::BadRequest("Idempotency-Key header is required".into()))?;
    let key = header_value
        .to_str()
        .map_err(|_| ApiError::BadRequest("Idempotency-Key header is invalid".into()))?;
    IdempotencyKey::try_from(key.to_string()).map_err(|e| ApiError::BadRequest(e.to_string()))
}
