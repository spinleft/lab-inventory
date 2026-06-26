use super::error::AttachmentError;
use super::model::{actor_for_user, fetch_attachment, validate_attachment_read_permission};
use crate::attachment_storage::AttachmentStorage;
use crate::domain::{AttachmentId, AttachmentStorageKey, UserId};
use actix_web::http::header;
use actix_web::{HttpResponse, web};
use anyhow::anyhow;
use sqlx::PgPool;

#[tracing::instrument(
    name = "Download attachment",
    skip(pool, storage),
    fields(actor_user_id=%actor_user_id, attachment_id=%attachment_id)
)]
pub async fn download_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<AttachmentStorage>,
    attachment_id: web::Path<AttachmentId>,
) -> Result<HttpResponse, AttachmentError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let attachment_id = attachment_id.into_inner();
    let row = fetch_attachment(&pool, *attachment_id)
        .await?
        .ok_or_else(|| AttachmentError::NotFound("Attachment not found".into()))?;
    validate_attachment_read_permission(&actor, &row)?;
    let storage_key = AttachmentStorageKey::parse(row.storage_key.clone())
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    let bytes = storage.read(&storage_key).await?;
    let content_type = row
        .mime_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_string());
    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, content_type))
        .insert_header((header::CONTENT_LENGTH, bytes.len().to_string()))
        .insert_header((
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                content_disposition_filename(&row.original_file_name)
            ),
        ))
        .body(bytes))
}

fn content_disposition_filename(file_name: &str) -> String {
    file_name
        .chars()
        .map(|ch| match ch {
            '"' | '\\' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect()
}
