use super::model::{LaboratoryResponse, create_laboratory_rollback_details};
use super::queries::{LaboratoryDatabaseError, insert_laboratory};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::UserId;
use crate::utils::{error_chain_fmt, required_text};
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    name: String,
    address: String,
    description: Option<String>,
    contact: Option<String>,
}

#[derive(thiserror::Error)]
pub enum CreateLaboratoryError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreateLaboratoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateLaboratoryError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateLaboratoryError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateLaboratoryError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateLaboratoryError::ConflictError(_) => StatusCode::CONFLICT,
            CreateLaboratoryError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<LaboratoryDatabaseError> for CreateLaboratoryError {
    fn from(error: LaboratoryDatabaseError) -> Self {
        match error {
            LaboratoryDatabaseError::Conflict(message) => Self::ConflictError(message),
            LaboratoryDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Create a laboratory",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, laboratory_name=%payload.name)
)]
pub async fn create_laboratory(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateLaboratoryError> {
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::Laboratory,
        Action::Create(Uuid::nil()),
    )
    .await?
    {
        return Err(CreateLaboratoryError::Forbidden(
            "You don't have permission to create laboratories.".into(),
        ));
    }

    let payload = payload.into_inner();
    let name =
        required_text(&payload.name, "name").map_err(CreateLaboratoryError::ValidationError)?;
    let address = required_text(&payload.address, "address")
        .map_err(CreateLaboratoryError::ValidationError)?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let laboratory = insert_laboratory(
        &mut transaction,
        name,
        address,
        payload.description.as_deref(),
        payload.contact.as_deref(),
    )
    .await?;

    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Create,
        AuditResource::Laboratory,
        Some(laboratory.laboratory_id),
        create_laboratory_rollback_details(&laboratory),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new laboratory.")?;

    Ok(HttpResponse::Created().json(LaboratoryResponse::from(laboratory)))
}
