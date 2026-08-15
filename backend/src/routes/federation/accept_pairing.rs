use super::queries::{
    FederationDatabaseError, consume_pairing_code, fetch_laboratory_identity, fetch_local_node_id,
    upsert_remote_node, upsert_trust,
};
use super::security::{FEDERATION_DISABLED, normalize_base_url, sha256_hex};
use crate::configuration::FederationSettings;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptPairingJsonData {
    pub pairing_code: String,
    pub requester_node_id: Uuid,
    pub requester_base_url: String,
    pub requester_laboratory_id: Uuid,
    pub requester_laboratory_name: String,
    pub shared_secret: String,
    pub tls_certificate_sha256: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct AcceptPairingResponse {
    pub node_id: Uuid,
    pub public_base_url: String,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub tls_certificate_sha256: Option<String>,
    pub key_version: i32,
}

#[derive(thiserror::Error)]
pub enum AcceptPairingError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for AcceptPairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for AcceptPairingError {
    fn status_code(&self) -> StatusCode {
        match self {
            AcceptPairingError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AcceptPairingError::Forbidden(_) => StatusCode::FORBIDDEN,
            AcceptPairingError::ConflictError(_) => StatusCode::CONFLICT,
            AcceptPairingError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<FederationDatabaseError> for AcceptPairingError {
    fn from(error: FederationDatabaseError) -> Self {
        match error {
            FederationDatabaseError::Validation(message) => Self::ValidationError(message),
            FederationDatabaseError::Conflict(message) => Self::ConflictError(message),
            FederationDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(name = "Accept federation pairing", skip(pool, settings, payload))]
pub async fn accept_pairing(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    payload: web::Json<AcceptPairingJsonData>,
) -> Result<HttpResponse, AcceptPairingError> {
    if !settings.enabled {
        return Err(AcceptPairingError::Forbidden(FEDERATION_DISABLED.into()));
    }
    let payload = payload.into_inner();
    let requester_base_url = normalize_base_url(&payload.requester_base_url, &settings)
        .map_err(AcceptPairingError::ValidationError)?;
    if payload.shared_secret.trim().is_empty() {
        return Err(AcceptPairingError::ValidationError(
            "shared_secret is required".into(),
        ));
    }
    let shared_secret_hash = sha256_hex(payload.shared_secret.as_bytes());
    let code_hash = sha256_hex(payload.pairing_code.as_bytes());

    let local_node_id = fetch_local_node_id(&pool)
        .await?
        .context("This server has no federation node identity")?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    // A code that is unknown, already spent or expired is all the same answer:
    // whoever is calling was not handed a usable one.
    let pairing_code = consume_pairing_code(&mut transaction, &code_hash)
        .await?
        .ok_or_else(|| {
            AcceptPairingError::ValidationError("Pairing code is invalid or has expired".into())
        })?;
    let laboratory = fetch_laboratory_identity(&pool, pairing_code.local_laboratory_id)
        .await?
        .context("The laboratory a pairing code was issued for no longer exists")?;
    let remote = upsert_remote_node(
        &mut transaction,
        payload.requester_node_id,
        &requester_base_url,
        Some(&payload.requester_laboratory_name),
        &payload.shared_secret,
        &shared_secret_hash,
        None,
        1,
    )
    .await?;
    upsert_trust(
        &mut transaction,
        pairing_code.local_laboratory_id,
        remote.remote_node_id,
        payload.requester_laboratory_id,
        Some(&payload.requester_laboratory_name),
        None,
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to accept a federation pairing")?;

    Ok(HttpResponse::Created().json(AcceptPairingResponse {
        node_id: local_node_id,
        public_base_url: settings.public_base_url.clone(),
        laboratory_id: laboratory.laboratory_id,
        laboratory_name: laboratory.name,
        tls_certificate_sha256: payload.tls_certificate_sha256,
        key_version: 1,
    }))
}
