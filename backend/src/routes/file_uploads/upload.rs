use super::queries::{FileUploadDatabaseError, insert_file_upload};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::domain::{FileName, LaboratoryId, UserId};
use crate::file_storage::FileStorage;
use crate::utils::error_chain_fmt;
use actix_multipart::Multipart;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use chrono::{Duration, Utc};
use futures_util::StreamExt;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum UploadFileError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for UploadFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UploadFileError {
    fn status_code(&self) -> StatusCode {
        match self {
            UploadFileError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UploadFileError::Forbidden(_) => StatusCode::FORBIDDEN,
            UploadFileError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<FileUploadDatabaseError> for UploadFileError {
    fn from(error: FileUploadDatabaseError) -> Self {
        match error {
            FileUploadDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Upload file",
    skip(pool, storage, payload),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn upload_file(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<FileStorage>,
    laboratory_id: web::Path<Uuid>,
    payload: Multipart,
) -> Result<HttpResponse, UploadFileError> {
    let laboratory_id: LaboratoryId = laboratory_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::FileUpload,
        Action::Create(laboratory_id.into()),
    )
    .await?
    {
        return Err(UploadFileError::Forbidden(
            "You don't have permission to upload files.".into(),
        ));
    }

    let upload = read_single_file(payload, storage.max_file_size_bytes()).await?;
    let stored = storage
        .store_upload(laboratory_id, &upload.original_file_name, &upload.bytes)
        .await?;

    // The blob is already on disk at this point, so a failed insert has to take
    // it back out rather than leave an orphan behind.
    let expires_at = Utc::now() + Duration::minutes(storage.upload_token_ttl_minutes() as i64);
    match insert_file_upload(
        &pool,
        laboratory_id,
        actor_user_id,
        upload.original_file_name.as_ref(),
        upload.mime_type.as_deref(),
        &stored,
        expires_at,
    )
    .await
    {
        Ok(response) => Ok(HttpResponse::Created().json(response)),
        Err(error) => {
            let _ = storage.delete(&stored.storage_key).await;
            Err(error.into())
        }
    }
}

struct MultipartUpload {
    original_file_name: FileName,
    mime_type: Option<String>,
    bytes: Vec<u8>,
}

async fn read_single_file(
    mut payload: Multipart,
    max_file_size_bytes: u64,
) -> Result<MultipartUpload, UploadFileError> {
    let mut upload = None;
    while let Some(field) = payload.next().await {
        let mut field = field
            .map_err(|e| UploadFileError::ValidationError(format!("Invalid multipart: {e}")))?;
        let content_disposition = field.content_disposition().cloned();
        let field_name = content_disposition
            .as_ref()
            .and_then(|value| value.get_name())
            .unwrap_or("");
        if field_name != "file" {
            return Err(UploadFileError::ValidationError(
                "Only multipart field `file` is supported".into(),
            ));
        }
        if upload.is_some() {
            return Err(UploadFileError::ValidationError(
                "Only one file can be uploaded at a time".into(),
            ));
        }
        let original_file_name = content_disposition
            .as_ref()
            .and_then(|value| value.get_filename())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "file".to_string());
        let original_file_name =
            FileName::parse(original_file_name).map_err(UploadFileError::ValidationError)?;
        let mime_type = field.content_type().map(ToString::to_string);
        let mut bytes = Vec::new();
        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_err(|e| {
                UploadFileError::ValidationError(format!("Failed to read multipart file: {e}"))
            })?;
            if bytes.len() as u64 + chunk.len() as u64 > max_file_size_bytes {
                return Err(UploadFileError::ValidationError(
                    "File upload exceeds configured size limit".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Err(UploadFileError::ValidationError(
                "File uploads cannot be empty".into(),
            ));
        }
        upload = Some(MultipartUpload {
            original_file_name,
            mime_type,
            bytes,
        });
    }
    upload.ok_or_else(|| {
        UploadFileError::ValidationError("Multipart field `file` is required".into())
    })
}
