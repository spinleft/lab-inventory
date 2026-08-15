use super::queries::{delete_file_upload_from_database, fetch_file_upload_for_update};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::{FileStorageKey, FileUploadId};
use crate::file_storage::FileStorage;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum DeleteFileUploadError {
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

impl std::fmt::Debug for DeleteFileUploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteFileUploadError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteFileUploadError::ValidationError(_) => StatusCode::BAD_REQUEST,
            DeleteFileUploadError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteFileUploadError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteFileUploadError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteFileUploadError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Delete file upload",
    skip(pool, storage),
    fields(actor_user_id=%laboratory_context.actor().user_id, upload_id=%upload_id)
)]
pub async fn delete_file_upload(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    storage: web::Data<FileStorage>,
    upload_id: FileUploadId,
) -> Result<HttpResponse, DeleteFileUploadError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::FileUpload,
        Action::Delete(upload_id.into()),
    )
    .await?
    {
        return Err(DeleteFileUploadError::Forbidden(
            "You are not allowed to delete this file upload.".into(),
        ));
    }
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let upload = fetch_file_upload_for_update(&mut transaction, upload_id)
        .await?
        .ok_or_else(|| DeleteFileUploadError::NotFound("File upload not found".into()))?;
    // Once consumed, the stored file backs a real attachment: deleting the
    // upload here would take the attachment's file with it.
    if upload.consumed_at.is_some() {
        return Err(DeleteFileUploadError::ConflictError(
            "File upload has already been assigned to an attachment".into(),
        ));
    }
    let storage_key = FileStorageKey::parse(upload.storage_key.clone())
        .map_err(|e| DeleteFileUploadError::UnexpectedError(anyhow!("{e}")))?;

    delete_file_upload_from_database(&mut transaction, upload.upload_id).await?;

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete file upload")?;
    storage.delete(&storage_key).await?;

    Ok(HttpResponse::NoContent().finish())
}
