use super::model::ConsumedFileUpload;
use super::model::FileUploadRow;
use crate::domain::FileUploadId;
use crate::utils::error_chain_fmt;
use actix_web::ResponseError;
use actix_web::http::StatusCode;
use chrono::Utc;
use sqlx::{Postgres, Transaction};

#[derive(thiserror::Error)]
pub enum ConsumeFileUploadError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ConsumeFileUploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ConsumeFileUploadError {
    fn status_code(&self) -> StatusCode {
        match self {
            ConsumeFileUploadError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ConsumeFileUploadError::NotFound(_) => StatusCode::NOT_FOUND,
            ConsumeFileUploadError::ConflictError(_) => StatusCode::CONFLICT,
            ConsumeFileUploadError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub async fn consume_file_upload(
    transaction: &mut Transaction<'_, Postgres>,
    upload_id: FileUploadId,
) -> Result<ConsumedFileUpload, ConsumeFileUploadError> {
    let upload = fetch_file_upload_for_update(transaction, upload_id)
        .await?
        .ok_or_else(|| ConsumeFileUploadError::NotFound("File upload not found".into()))?;
    // Re-checked here rather than only at the permission layer: that check runs
    // outside this transaction and without the row lock, so two concurrent
    // requests could both pass it and consume the same upload twice.
    if upload.consumed_at.is_some() {
        return Err(ConsumeFileUploadError::ConflictError(
            "File upload has already been consumed".into(),
        ));
    }
    if Utc::now() > upload.expires_at {
        return Err(ConsumeFileUploadError::ValidationError(
            "File upload has expired".into(),
        ));
    }
    sqlx::query(
        r#"
        UPDATE file_uploads
        SET consumed_at = now()
        WHERE upload_id = $1
        "#,
    )
    .bind(upload.upload_id)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| ConsumeFileUploadError::UnexpectedError(e.into()))?;

    Ok(upload.into())
}

async fn fetch_file_upload_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    upload_id: FileUploadId,
) -> Result<Option<FileUploadRow>, ConsumeFileUploadError> {
    sqlx::query_as::<_, FileUploadRow>(
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
    )
    .bind(*upload_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| ConsumeFileUploadError::UnexpectedError(e.into()))
}
