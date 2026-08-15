use super::queries::{FederationDatabaseError, fetch_local_node_id, insert_pairing_code};
use super::security::{FEDERATION_DISABLED, generate_token, sha256_hex};
use super::service::MANAGE_FEDERATION_FORBIDDEN;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::configuration::FederationSettings;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use chrono::{Duration, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize)]
struct PairingCodeResponse {
    pairing_code_id: Uuid,
    pairing_code: String,
    expires_at: chrono::DateTime<Utc>,
    local_node_id: Uuid,
    local_base_url: String,
    local_laboratory_id: Uuid,
}

#[derive(thiserror::Error)]
pub enum CreatePairingCodeError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreatePairingCodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreatePairingCodeError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreatePairingCodeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreatePairingCodeError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreatePairingCodeError::ConflictError(_) => StatusCode::CONFLICT,
            CreatePairingCodeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<FederationDatabaseError> for CreatePairingCodeError {
    fn from(error: FederationDatabaseError) -> Self {
        match error {
            FederationDatabaseError::Validation(message) => Self::ValidationError(message),
            FederationDatabaseError::Conflict(message) => Self::ConflictError(message),
            FederationDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Create federation pairing code",
    skip(pool, settings),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn create_pairing_code(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_context: LaboratoryContext,
) -> Result<HttpResponse, CreatePairingCodeError> {
    if !settings.enabled {
        return Err(CreatePairingCodeError::Forbidden(
            FEDERATION_DISABLED.into(),
        ));
    }
    let authorization_actor = laboratory_context.authorization_actor();
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    if !validate_permission(
        &pool,
        &authorization_actor,
        ResourceType::Federation,
        Action::Create(laboratory_id),
    )
    .await?
    {
        return Err(CreatePairingCodeError::Forbidden(
            MANAGE_FEDERATION_FORBIDDEN.into(),
        ));
    }
    let actor = laboratory_context.actor();
    let local_node_id = fetch_local_node_id(&pool)
        .await?
        .context("This server has no federation node identity")?;
    let code = generate_token(24);
    let code_hash = sha256_hex(code.as_bytes());
    let expires_at = Utc::now() + Duration::minutes(15);
    let row =
        insert_pairing_code(&pool, laboratory_id, &code_hash, expires_at, *actor.user_id).await?;

    Ok(HttpResponse::Created().json(PairingCodeResponse {
        pairing_code_id: row.pairing_code_id,
        pairing_code: code,
        expires_at: row.expires_at,
        local_node_id,
        local_base_url: settings.public_base_url.clone(),
        local_laboratory_id: row.local_laboratory_id,
    }))
}
