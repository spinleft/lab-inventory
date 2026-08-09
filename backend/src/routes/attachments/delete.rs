use super::model::{delete_attachment_rollback_details, fetch_attachment_for_update};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AttachmentId, FileStorageKey, UserId};
use crate::file_storage::FileStorage;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum DeleteAttachmentError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for DeleteAttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteAttachmentError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteAttachmentError::ValidationError(_) => StatusCode::BAD_REQUEST,
            DeleteAttachmentError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteAttachmentError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Delete attachment",
    skip(pool, storage),
    fields(actor_user_id=%actor_user_id, attachment_id=%attachment_id)
)]
pub async fn delete_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<FileStorage>,
    attachment_id: web::Path<Uuid>,
) -> Result<HttpResponse, DeleteAttachmentError> {
    let attachment_id: AttachmentId = attachment_id.into_inner().into();
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_attachment_for_update(&mut transaction, attachment_id)
        .await?
        .ok_or_else(|| DeleteAttachmentError::NotFound("Attachment not found".into()))?;
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::AttachmentAssignment,
        Action::Delete(attachment_id.into()),
    )
    .await?
    {
        return Err(DeleteAttachmentError::Forbidden(
            "You are not allowed to delete this attachment".into(),
        ));
    }
    let storage_key = FileStorageKey::parse(existing.storage_key.clone())
        .map_err(|e| DeleteAttachmentError::UnexpectedError(anyhow!("{e}")))?;

    delete_attachment_from_database(&mut transaction, attachment_id).await?;
    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Delete,
        AuditResource::Attachment,
        Some(existing.attachment_id),
        delete_attachment_rollback_details(&existing),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete attachment")?;
    storage.delete(&storage_key).await?;

    Ok(HttpResponse::NoContent().finish())
}

#[tracing::instrument(name = "Deleting attachment from the database", skip(transaction), fields(attachment_id=%attachment_id))]
async fn delete_attachment_from_database(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    attachment_id: AttachmentId,
) -> Result<(), DeleteAttachmentError> {
    sqlx::query!(
        r#"
            WITH deleted_assignment AS (
                DELETE FROM asset_attachment_assignments
                WHERE attachment_id = $1
                RETURNING file_id
            )
            DELETE FROM files
            WHERE file_id IN (SELECT file_id FROM deleted_assignment)
        "#,
        Uuid::from(attachment_id)
    )
    .execute(transaction.as_mut())
    .await
    .map_err(|e| DeleteAttachmentError::UnexpectedError(e.into()))?;

    Ok(())
}
