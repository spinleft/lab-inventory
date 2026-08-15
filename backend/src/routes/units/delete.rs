use super::model::delete_unit_rollback_details;
use super::queries::{UnitDatabaseError, delete_unit_from_database, fetch_unit_for_update};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::UnitId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum DeleteUnitError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for DeleteUnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteUnitError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteUnitError::ValidationError(_) => StatusCode::BAD_REQUEST,
            DeleteUnitError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteUnitError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteUnitError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteUnitError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<UnitDatabaseError> for DeleteUnitError {
    fn from(error: UnitDatabaseError) -> Self {
        match error {
            UnitDatabaseError::Validation(message) => Self::ValidationError(message),
            UnitDatabaseError::Conflict(message) => Self::ConflictError(message),
            UnitDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Delete a unit",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, unit_id=%unit_id)
)]
pub async fn delete_unit(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    unit_id: UnitId,
) -> Result<HttpResponse, DeleteUnitError> {
    let actor = laboratory_context.authorization_actor();
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_unit_for_update(&mut transaction, *unit_id)
        .await?
        .ok_or(DeleteUnitError::NotFound("Unit not found".into()))?;
    if !validate_permission(&pool, &actor, ResourceType::Unit, Action::Delete(*unit_id)).await? {
        return Err(DeleteUnitError::NotFound("Unit not found".into()));
    }
    delete_unit_from_database(&mut transaction, existing.unit_id).await?;
    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Delete,
        AuditResource::Unit,
        Some(existing.unit_id),
        delete_unit_rollback_details(&existing),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete a unit.")?;

    Ok(HttpResponse::NoContent().finish())
}
