use super::helpers::{ensure_can_write, fetch_attachment, map_database_error};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Delete attachment metadata", skip(pool), fields(user_id=%user_id, attachment_id=%attachment_id))]
pub async fn delete_attachment(
    user_id: UserId,
    pool: web::Data<PgPool>,
    attachment_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let attachment_id = attachment_id.into_inner();
    let attachment = fetch_attachment(pool.get_ref(), attachment_id).await?;
    ensure_can_write(&actor, attachment.laboratory_id)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    sqlx::query("DELETE FROM attachments WHERE attachment_id = $1")
        .bind(attachment_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
    record_audit(
        &mut transaction,
        &actor,
        Some(attachment.laboratory_id),
        AuditAction::Delete,
        AuditResource::Attachment,
        Some(attachment_id),
        json!({ "resource_type": attachment.resource_type, "resource_id": attachment.resource_id }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::NoContent().finish())
}
