use crate::routes::fetch_remote_laboratory_secret;
use crate::utils::ApiError;
use actix_web::HttpRequest;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::header::HeaderValue;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct FederationActor {
    pub remote_laboratory_id: Uuid,
}

pub async fn verify_federation_request(
    pool: &PgPool,
    request: &HttpRequest,
    body: &[u8],
) -> Result<FederationActor, ApiError> {
    let remote_laboratory_id = parse_uuid_header(request, "X-Lab-Id")?;
    let key_id = required_header(request, "X-Key-Id")?;
    let timestamp = required_header(request, "X-Timestamp")?;
    let nonce = required_header(request, "X-Nonce")?;
    let signature = required_header(request, "X-Signature")?;
    let remote = fetch_remote_laboratory_secret(pool, remote_laboratory_id).await?;
    if !remote.is_enabled || remote.key_id != key_id {
        return Err(ApiError::Unauthorized);
    }
    validate_timestamp(timestamp)?;
    remember_nonce(pool, remote_laboratory_id, nonce).await?;

    let expected = sign(
        request.method().as_str(),
        request
            .uri()
            .path_and_query()
            .map(|path| path.as_str())
            .unwrap_or(request.path()),
        timestamp,
        nonce,
        body,
        &remote.shared_secret,
    )?;
    if expected != signature {
        return Err(ApiError::Unauthorized);
    }

    sqlx::query(
        "UPDATE remote_laboratories SET last_seen_at = now() WHERE remote_laboratory_id = $1",
    )
    .bind(remote_laboratory_id)
    .execute(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(FederationActor {
        remote_laboratory_id,
    })
}

pub fn signed_headers(
    method: &str,
    path_and_query: &str,
    body: &[u8],
    local_laboratory_id: Uuid,
    key_id: &str,
    shared_secret: &str,
) -> Result<reqwest::header::HeaderMap, ApiError> {
    let timestamp = Utc::now().to_rfc3339();
    let nonce = Uuid::new_v4().to_string();
    let signature = sign(
        method,
        path_and_query,
        &timestamp,
        &nonce,
        body,
        shared_secret,
    )?;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "X-Lab-Id",
        HeaderValue::from_str(&local_laboratory_id.to_string())
            .map_err(|e| ApiError::UnexpectedError(anyhow::anyhow!(e)))?,
    );
    headers.insert(
        "X-Key-Id",
        HeaderValue::from_str(key_id).map_err(|e| ApiError::UnexpectedError(anyhow::anyhow!(e)))?,
    );
    headers.insert(
        "X-Timestamp",
        HeaderValue::from_str(&timestamp)
            .map_err(|e| ApiError::UnexpectedError(anyhow::anyhow!(e)))?,
    );
    headers.insert(
        "X-Nonce",
        HeaderValue::from_str(&nonce).map_err(|e| ApiError::UnexpectedError(anyhow::anyhow!(e)))?,
    );
    headers.insert(
        "X-Signature",
        HeaderValue::from_str(&signature)
            .map_err(|e| ApiError::UnexpectedError(anyhow::anyhow!(e)))?,
    );
    Ok(headers)
}

fn required_header<'a>(request: &'a HttpRequest, name: &str) -> Result<&'a str, ApiError> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or(ApiError::Unauthorized)
}

fn parse_uuid_header(request: &HttpRequest, name: &str) -> Result<Uuid, ApiError> {
    required_header(request, name)?
        .parse()
        .map_err(|_| ApiError::Unauthorized)
}

fn validate_timestamp(timestamp: &str) -> Result<(), ApiError> {
    let timestamp = DateTime::parse_from_rfc3339(timestamp)
        .map_err(|_| ApiError::Unauthorized)?
        .with_timezone(&Utc);
    let age = Utc::now()
        .signed_duration_since(timestamp)
        .num_seconds()
        .abs();
    if age > 900 {
        return Err(ApiError::Unauthorized);
    }
    Ok(())
}

async fn remember_nonce(
    pool: &PgPool,
    remote_laboratory_id: Uuid,
    nonce: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO federation_nonces (remote_laboratory_id, nonce)
        VALUES ($1, $2)
        "#,
    )
    .bind(remote_laboratory_id)
    .bind(nonce)
    .execute(pool)
    .await
    .map_err(|error| {
        if let sqlx::Error::Database(database_error) = &error
            && let Some("23505") = database_error.code().as_deref()
        {
            return ApiError::Unauthorized;
        }
        ApiError::UnexpectedError(error.into())
    })?;
    Ok(())
}

fn sign(
    method: &str,
    path_and_query: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
    shared_secret: &str,
) -> Result<String, ApiError> {
    let body_hash = hex::encode(Sha256::digest(body));
    let signing_string = format!("{method}\n{path_and_query}\n{timestamp}\n{nonce}\n{body_hash}");
    let mut mac = HmacSha256::new_from_slice(shared_secret.as_bytes())
        .map_err(|e| ApiError::UnexpectedError(anyhow::anyhow!(e)))?;
    mac.update(signing_string.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}
