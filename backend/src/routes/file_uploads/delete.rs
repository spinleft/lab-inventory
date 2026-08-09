use super::model::FileUploadRow;
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::domain::{FileStorageKey, FileUploadId, UserId};
use crate::file_storage::FileStorage;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

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
    fields(actor_user_id=%actor_user_id, upload_id=%upload_id)
)]
pub async fn delete_file_upload(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<FileStorage>,
    upload_id: web::Path<FileUploadId>,
) -> Result<HttpResponse, DeleteFileUploadError> {
    let upload_id = upload_id.into_inner();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::FileUpload,
        Action::Delete(upload_id.into()),
    )
    .await?
    {
        return Err(DeleteFileUploadError::Forbidden(
            "You don't have permission to delete files.".into(),
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

    sqlx::query(
        r#"
        DELETE FROM file_uploads
        WHERE upload_id = $1
        "#,
    )
    .bind(upload.upload_id)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| DeleteFileUploadError::UnexpectedError(e.into()))?;

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete file upload")?;
    storage.delete(&storage_key).await?;

    Ok(HttpResponse::NoContent().finish())
}

async fn fetch_file_upload_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    upload_id: FileUploadId,
) -> Result<Option<FileUploadRow>, DeleteFileUploadError> {
    sqlx::query_as!(
        FileUploadRow,
        r#"
        SELECT
            upload_id,
            laboratory_id,
            storage_backend,
            storage_key,
            original_file_name,
            mime_type,
            file_size_bytes,
            sha256_hex,
            uploaded_by_user_id,
            expires_at,
            consumed_at
        FROM file_uploads
        WHERE upload_id = $1
        FOR UPDATE
        "#,
        Uuid::from(upload_id)
    )
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| DeleteFileUploadError::UnexpectedError(e.into()))
}
