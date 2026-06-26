use super::error::AttachmentError;
use super::model::{AttachmentUploadResponse, actor_for_user, validate_write_permission};
use crate::attachment_storage::{AttachmentStorage, StoredFile};
use crate::domain::{AttachmentFileName, LaboratoryId, UserId};
use actix_multipart::Multipart;
use actix_web::{HttpResponse, web};
use anyhow::anyhow;
use chrono::{Duration, Utc};
use futures_util::StreamExt;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(
    name = "Upload attachment file",
    skip(pool, storage, payload),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn upload_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<AttachmentStorage>,
    laboratory_id: web::Path<Uuid>,
    payload: Multipart,
) -> Result<HttpResponse, AttachmentError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id.into_inner())
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    let actor = actor_for_user(&pool, actor_user_id).await?;
    validate_write_permission(&actor, &laboratory_id)?;

    let upload = read_single_file(payload, storage.max_file_size_bytes()).await?;
    let stored = storage
        .store_upload(*laboratory_id, &upload.original_file_name, &upload.bytes)
        .await?;

    match insert_attachment_upload(
        &pool,
        *laboratory_id,
        *actor.user_id,
        &upload,
        &stored,
        storage.upload_token_ttl_minutes(),
    )
    .await
    {
        Ok(response) => Ok(HttpResponse::Created().json(response)),
        Err(error) => {
            let _ = storage.delete(&stored.storage_key).await;
            Err(error)
        }
    }
}

struct MultipartUpload {
    original_file_name: AttachmentFileName,
    mime_type: Option<String>,
    bytes: Vec<u8>,
}

async fn read_single_file(
    mut payload: Multipart,
    max_file_size_bytes: u64,
) -> Result<MultipartUpload, AttachmentError> {
    let mut upload = None;
    while let Some(field) = payload.next().await {
        let mut field = field
            .map_err(|e| AttachmentError::ValidationError(format!("Invalid multipart: {e}")))?;
        let content_disposition = field.content_disposition().cloned();
        let field_name = content_disposition
            .as_ref()
            .and_then(|value| value.get_name())
            .unwrap_or("");
        if field_name != "file" {
            return Err(AttachmentError::ValidationError(
                "Only multipart field `file` is supported".into(),
            ));
        }
        if upload.is_some() {
            return Err(AttachmentError::ValidationError(
                "Only one file can be uploaded at a time".into(),
            ));
        }
        let original_file_name = content_disposition
            .as_ref()
            .and_then(|value| value.get_filename())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "attachment".to_string());
        let original_file_name = AttachmentFileName::parse(original_file_name)
            .map_err(AttachmentError::ValidationError)?;
        let mime_type = field.content_type().map(ToString::to_string);
        let mut bytes = Vec::new();
        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_err(|e| {
                AttachmentError::ValidationError(format!("Failed to read multipart file: {e}"))
            })?;
            if bytes.len() as u64 + chunk.len() as u64 > max_file_size_bytes {
                return Err(AttachmentError::ValidationError(
                    "Attachment file exceeds configured size limit".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Err(AttachmentError::ValidationError(
                "Attachment files cannot be empty".into(),
            ));
        }
        upload = Some(MultipartUpload {
            original_file_name,
            mime_type,
            bytes,
        });
    }
    upload.ok_or_else(|| {
        AttachmentError::ValidationError("Multipart field `file` is required".into())
    })
}

async fn insert_attachment_upload(
    pool: &PgPool,
    laboratory_id: Uuid,
    uploaded_by_user_id: Uuid,
    upload: &MultipartUpload,
    stored: &StoredFile,
    ttl_minutes: i64,
) -> Result<AttachmentUploadResponse, AttachmentError> {
    let upload_id = Uuid::new_v4();
    let expires_at = Utc::now() + Duration::minutes(ttl_minutes);
    sqlx::query_as::<_, AttachmentUploadResponse>(
        r#"
        INSERT INTO attachment_uploads (
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
    .bind(upload_id)
    .bind(laboratory_id)
    .bind(stored.storage_backend.as_str())
    .bind(stored.storage_key.as_ref())
    .bind(upload.original_file_name.as_ref())
    .bind(upload.mime_type.as_deref())
    .bind(stored.file_size_bytes.as_i64())
    .bind(stored.sha256_hex.as_ref())
    .bind(uploaded_by_user_id)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}
