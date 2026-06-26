use super::error::AttachmentError;
use super::model::{AttachmentUploadRow, actor_for_user};
use crate::attachment_storage::AttachmentStorage;
use crate::domain::{AttachmentStorageKey, AttachmentUploadId, UserId};
use actix_web::{HttpResponse, web};
use anyhow::{Context, anyhow};
use sqlx::{PgPool, Postgres, Transaction};

#[tracing::instrument(
    name = "Delete attachment upload",
    skip(pool, storage),
    fields(actor_user_id=%actor_user_id, upload_id=%upload_id)
)]
pub async fn delete_attachment_upload(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<AttachmentStorage>,
    upload_id: web::Path<AttachmentUploadId>,
) -> Result<HttpResponse, AttachmentError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let upload_id = upload_id.into_inner();
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let upload = fetch_upload_for_update(&mut transaction, upload_id)
        .await?
        .ok_or_else(|| AttachmentError::NotFound("Attachment upload not found".into()))?;
    if upload.uploaded_by_user_id != Some(*actor.user_id) {
        return Err(AttachmentError::Forbidden(
            "You can only delete attachment uploads created by your own user".into(),
        ));
    }
    if upload.consumed_at.is_some() {
        return Err(AttachmentError::ConflictError(
            "Attachment upload has already been consumed".into(),
        ));
    }
    let storage_key = AttachmentStorageKey::parse(upload.storage_key.clone())
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;

    sqlx::query(
        r#"
        DELETE FROM attachment_uploads
        WHERE upload_id = $1
        "#,
    )
    .bind(*upload_id)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))?;

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete attachment upload")?;
    storage.delete(&storage_key).await?;

    Ok(HttpResponse::NoContent().finish())
}

async fn fetch_upload_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    upload_id: AttachmentUploadId,
) -> Result<Option<AttachmentUploadRow>, AttachmentError> {
    sqlx::query_as::<_, AttachmentUploadRow>(
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
        FROM attachment_uploads
        WHERE upload_id = $1
        FOR UPDATE
        "#,
    )
    .bind(*upload_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}
