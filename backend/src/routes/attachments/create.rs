use super::helpers::{
    ensure_can_write, fetch_attachment_in_transaction, map_database_error, normalize_resource_type,
    normalize_visibility, required_text, resolve_resource_laboratory,
};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    resource_type: String,
    resource_id: Uuid,
    file_name: String,
    mime_type: Option<String>,
    file_size_bytes: i64,
    storage_url: String,
    visibility: Option<String>,
}

#[tracing::instrument(name = "Create attachment metadata", skip(pool, payload), fields(user_id=%user_id))]
pub async fn create_attachment(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let resource_type = normalize_resource_type(&payload.resource_type)?;
    let visibility = normalize_visibility(payload.visibility.as_deref())?;
    let file_name = required_text(&payload.file_name, "file_name")?;
    let storage_url = required_text(&payload.storage_url, "storage_url")?;
    if payload.file_size_bytes < 0 {
        return Err(ApiError::BadRequest(
            "file_size_bytes must be non-negative".into(),
        ));
    }
    let laboratory_id =
        resolve_resource_laboratory(pool.get_ref(), &actor, resource_type, payload.resource_id)
            .await?;
    ensure_can_write(&actor, laboratory_id)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let attachment_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO attachments (
            attachment_id,
            laboratory_id,
            resource_type,
            resource_id,
            file_name,
            mime_type,
            file_size_bytes,
            storage_url,
            visibility,
            uploaded_by_user_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(attachment_id)
    .bind(laboratory_id)
    .bind(resource_type)
    .bind(payload.resource_id)
    .bind(file_name)
    .bind(payload.mime_type.as_deref())
    .bind(payload.file_size_bytes)
    .bind(storage_url)
    .bind(visibility)
    .bind(actor.user_id)
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(laboratory_id),
        AuditAction::Create,
        AuditResource::Attachment,
        Some(attachment_id),
        json!({ "resource_type": resource_type, "resource_id": payload.resource_id }),
    )
    .await?;
    let attachment = fetch_attachment_in_transaction(&mut transaction, attachment_id).await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(attachment))
}
