use super::model::{delete_laboratory_rollback_details, fetch_laboratory};
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
pub enum DeleteLaboratoryError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for DeleteLaboratoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteLaboratoryError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteLaboratoryError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteLaboratoryError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteLaboratoryError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteLaboratoryError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Delete a laboratory",
    skip(pool),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn delete_laboratory(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, DeleteLaboratoryError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(DeleteLaboratoryError::UnexpectedError)?
        .ok_or(DeleteLaboratoryError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_delete_permission(&actor)?;

    let existing =
        fetch_laboratory(&pool, *laboratory_id)
            .await?
            .ok_or(DeleteLaboratoryError::NotFound(
                "Laboratory not found".into(),
            ))?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    delete_laboratory_from_database(&mut transaction, existing.laboratory_id).await?;
    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Delete,
        AuditResource::Laboratory,
        Some(existing.laboratory_id),
        delete_laboratory_rollback_details(&existing),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete a laboratory.")?;

    Ok(HttpResponse::NoContent().finish())
}

fn validate_delete_permission(actor: &Actor) -> Result<(), DeleteLaboratoryError> {
    if actor.is_root() || actor.is_super_admin() {
        Ok(())
    } else {
        Err(DeleteLaboratoryError::Forbidden(
            "You don't have permission to delete laboratories.".into(),
        ))
    }
}

#[tracing::instrument(
    name = "Deleting laboratory from the database",
    skip(transaction),
    fields(laboratory_id=%laboratory_id)
)]
async fn delete_laboratory_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
) -> Result<(), DeleteLaboratoryError> {
    sqlx::query!(
        "DELETE FROM laboratories WHERE laboratory_id = $1",
        laboratory_id
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

fn map_database_error(error: sqlx::Error) -> DeleteLaboratoryError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23503") {
            return DeleteLaboratoryError::ConflictError(
                "Laboratory is referenced by other records".into(),
            );
        }
    }

    DeleteLaboratoryError::UnexpectedError(error.into())
}
