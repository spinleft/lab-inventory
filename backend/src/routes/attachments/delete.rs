use super::error::AttachmentError;
use super::model::{
    DeletedAttachmentRow, actor_for_user, attachment_audit_json, fetch_attachment_for_update,
    validate_write_permission,
};
use crate::attachment_storage::AttachmentStorage;
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AttachmentId, AttachmentStorageKey, LaboratoryId, UserId};
use actix_web::{HttpResponse, web};
use anyhow::{Context, anyhow};
use serde_json::json;
use sqlx::PgPool;

#[tracing::instrument(
    name = "Delete attachment",
    skip(pool, storage),
    fields(actor_user_id=%actor_user_id, attachment_id=%attachment_id)
)]
pub async fn delete_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<AttachmentStorage>,
    attachment_id: web::Path<AttachmentId>,
) -> Result<HttpResponse, AttachmentError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let attachment_id = attachment_id.into_inner();
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let row = fetch_attachment_for_update(&mut transaction, *attachment_id)
        .await?
        .ok_or_else(|| AttachmentError::NotFound("Attachment not found".into()))?;
    let laboratory_id = LaboratoryId::parse(row.laboratory_id)
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    validate_write_permission(&actor, &laboratory_id)?;
    let storage_key = AttachmentStorageKey::parse(row.storage_key.clone())
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;

    sqlx::query(
        r#"
        UPDATE attachments
        SET deleted_at = now(),
            deleted_by_user_id = $2,
            updated_at = now()
        WHERE attachment_id = $1
          AND deleted_at IS NULL
        "#,
    )
    .bind(row.attachment_id)
    .bind(*actor.user_id)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))?;

    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Delete,
        AuditResource::Attachment,
        Some(row.attachment_id),
        json!({
            "deleted": attachment_audit_json(&row),
        }),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete attachment")?;
    storage.delete(&storage_key).await?;

    Ok(HttpResponse::NoContent().finish())
}

pub(crate) async fn delete_storage_objects(
    storage: &AttachmentStorage,
    rows: &[DeletedAttachmentRow],
) -> Result<(), anyhow::Error> {
    for row in rows {
        let storage_key =
            AttachmentStorageKey::parse(row.storage_key.clone()).map_err(|e| anyhow!(e))?;
        storage.delete(&storage_key).await?;
    }
    Ok(())
}
