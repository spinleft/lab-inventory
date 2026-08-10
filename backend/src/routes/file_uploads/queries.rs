//! Every SQL statement the file upload routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - functions never return a handler error type, only
//!   [`FileUploadDatabaseError`], so any handler can reuse them
use super::model::{FileUploadResponse, FileUploadRow};
use crate::domain::{FileUploadId, LaboratoryId, StoredFile, UserId};
use crate::utils::error_chain_fmt;
use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(super) enum FileUploadDatabaseError {
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for FileUploadDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

/// Takes the row lock every write path needs: whether an upload may still be
/// consumed or deleted depends on `consumed_at`, which two requests could
/// otherwise read at the same time.
pub(super) async fn fetch_file_upload_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    upload_id: FileUploadId,
) -> Result<Option<FileUploadRow>, anyhow::Error> {
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
    .context("Failed to fetch file upload for update")
}

#[tracing::instrument(
    name = "Saving new file upload in the database",
    skip(pool, stored, original_file_name, mime_type),
    fields(laboratory_id=%laboratory_id)
)]
pub(super) async fn insert_file_upload(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    uploaded_by_user_id: UserId,
    original_file_name: &str,
    mime_type: Option<&str>,
    stored: &StoredFile,
    expires_at: DateTime<Utc>,
) -> Result<FileUploadResponse, FileUploadDatabaseError> {
    sqlx::query_as::<_, FileUploadResponse>(
        r#"
        INSERT INTO file_uploads (
            upload_id,
            laboratory_id,
            storage_backend,
            storage_key,
            original_file_name,
            mime_type,
            file_size_bytes,
            sha256_hex,
            uploaded_by_user_id,
            expires_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING
            upload_id,
            laboratory_id,
            original_file_name,
            mime_type,
            file_size_bytes,
            sha256_hex,
            expires_at,
            created_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::from(laboratory_id))
    .bind(stored.storage_backend.as_str())
    .bind(stored.storage_key.as_ref())
    .bind(original_file_name)
    .bind(mime_type)
    .bind(stored.file_size_bytes.as_i64())
    .bind(stored.sha256_hex.as_ref())
    .bind(Uuid::from(uploaded_by_user_id))
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(|e| FileUploadDatabaseError::Unexpected(e.into()))
}

/// Marks the upload as spent so it cannot be turned into a second attachment.
pub(super) async fn mark_file_upload_consumed(
    transaction: &mut Transaction<'_, Postgres>,
    upload_id: Uuid,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        UPDATE file_uploads
        SET consumed_at = now()
        WHERE upload_id = $1
        "#,
        upload_id,
    )
    .execute(transaction.as_mut())
    .await
    .context("Failed to mark file upload as consumed")?;

    Ok(())
}

#[tracing::instrument(
    name = "Deleting file upload from the database",
    skip(transaction),
    fields(upload_id=%upload_id)
)]
pub(super) async fn delete_file_upload_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    upload_id: Uuid,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        DELETE FROM file_uploads
        WHERE upload_id = $1
        "#,
        upload_id,
    )
    .execute(transaction.as_mut())
    .await
    .context("Failed to delete file upload")?;

    Ok(())
}
