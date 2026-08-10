//! Business flows that chain several statements together.
//!
//! Anything here orchestrates `queries.rs` and enforces rules that span more
//! than one row or table. Single-statement work belongs in `queries.rs`; HTTP
//! concerns belong in the handler modules.
//!
//! [`consume_file_upload`] is the flow other route modules reach for: every
//! attachment starts life as an upload that is spent here, so its error type
//! lives with it rather than in a handler module.
use super::model::ConsumedFileUpload;
use super::queries::{fetch_file_upload_for_update, mark_file_upload_consumed};
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

/// Spends an upload, returning the stored file it points at.
///
/// The upload row stays behind marked as consumed rather than being deleted, so
/// a retry of the same request is rejected instead of silently uploading the
/// file twice.
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
    mark_file_upload_consumed(transaction, upload.upload_id).await?;

    Ok(upload.into())
}
