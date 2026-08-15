use super::model::delete_attachment_rollback_details;
use super::queries::{
    AttachmentDatabaseError, delete_attachment_from_database, fetch_attachment_for_update,
};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AttachmentId, FileStorageKey};
use crate::file_storage::FileStorage;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum DeleteAttachmentError {
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

impl From<AttachmentDatabaseError> for DeleteAttachmentError {
    fn from(error: AttachmentDatabaseError) -> Self {
        match error {
            AttachmentDatabaseError::Validation(message) => Self::ValidationError(message),
            AttachmentDatabaseError::Conflict(message) => Self::ConflictError(message),
            AttachmentDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
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
            DeleteAttachmentError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Delete attachment",
    skip(pool, storage),
    fields(actor_user_id=%laboratory_context.actor().user_id, attachment_id=%attachment_id)
)]
pub async fn delete_attachment(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    storage: web::Data<FileStorage>,
    attachment_id: AttachmentId,
) -> Result<HttpResponse, DeleteAttachmentError> {
    let actor = laboratory_context.authorization_actor();
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_attachment_for_update(&mut transaction, attachment_id)
        .await?
        .ok_or_else(|| DeleteAttachmentError::NotFound("Attachment not found".into()))?;
    if !validate_permission(
        &pool,
        &actor,
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
        laboratory_context.actor(),
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
