use super::model::{delete_unit_rollback_details, fetch_unit_for_update};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::UserId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum DeleteUnitError {
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
            DeleteUnitError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteUnitError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteUnitError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteUnitError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Delete a unit",
    skip(pool),
    fields(actor_user_id=%actor_user_id, unit_id=%unit_id)
)]
pub async fn delete_unit(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    unit_id: web::Path<Uuid>,
) -> Result<HttpResponse, DeleteUnitError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(DeleteUnitError::UnexpectedError)?
        .ok_or(DeleteUnitError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_delete_permission(&actor)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_unit_for_update(&mut transaction, *unit_id)
        .await?
        .ok_or(DeleteUnitError::NotFound("Unit not found".into()))?;
    delete_unit_from_database(&mut transaction, existing.unit_id).await?;
    record_audit(
        &mut transaction,
        &actor,
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

fn validate_delete_permission(actor: &Actor) -> Result<(), DeleteUnitError> {
    if actor.can_manage_units() {
        Ok(())
    } else {
        Err(DeleteUnitError::Forbidden(
            "You don't have permission to delete units.".into(),
        ))
    }
}

#[tracing::instrument(name = "Deleting unit from the database", skip(transaction), fields(unit_id=%unit_id))]
async fn delete_unit_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> Result<(), DeleteUnitError> {
    sqlx::query!("DELETE FROM units WHERE unit_id = $1", unit_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;

    Ok(())
}

fn map_database_error(error: sqlx::Error) -> DeleteUnitError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23503") {
            return DeleteUnitError::ConflictError("Unit is referenced by other records".into());
        }
    }

    DeleteUnitError::UnexpectedError(error.into())
}
