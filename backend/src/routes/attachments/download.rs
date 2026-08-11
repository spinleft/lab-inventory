use super::queries::fetch_attachment_file;
use crate::access_control::AttachmentPathId;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::{AttachmentId, FileStorageKey};
use crate::file_storage::FileStorage;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::http::header;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::anyhow;
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum DownloadAttachmentError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for DownloadAttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DownloadAttachmentError {
    fn status_code(&self) -> StatusCode {
        match self {
            DownloadAttachmentError::ValidationError(_) => StatusCode::BAD_REQUEST,
            DownloadAttachmentError::Forbidden(_) => StatusCode::FORBIDDEN,
            DownloadAttachmentError::NotFound(_) => StatusCode::NOT_FOUND,
            DownloadAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Download attachment",
    skip(pool, storage),
    fields(actor_user_id=%laboratory_context.actor().user_id, attachment_id=%attachment_id)
)]
pub async fn download_attachment(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    storage: web::Data<FileStorage>,
    attachment_id: AttachmentPathId,
) -> Result<HttpResponse, DownloadAttachmentError> {
    let actor = laboratory_context.authorization_actor();
    let attachment_id: AttachmentId = attachment_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::AttachmentAssignment,
        Action::Read(attachment_id.into()),
    )
    .await?
    {
        return Err(DownloadAttachmentError::Forbidden(
            "You do not have permission to download this attachment".into(),
        ));
    }

    let row = fetch_attachment_file(&pool, attachment_id)
        .await?
        .ok_or_else(|| DownloadAttachmentError::NotFound("Attachment not found".into()))?;

    let storage_key = FileStorageKey::parse(row.storage_key.clone())
        .map_err(|e| DownloadAttachmentError::UnexpectedError(anyhow!("{e}")))?;
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
