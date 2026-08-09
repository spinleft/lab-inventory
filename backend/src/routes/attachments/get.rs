use super::model::{AttachmentResponse, AttachmentRow};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::domain::{AttachmentId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum GetAttachmentError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GetAttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GetAttachmentError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetAttachmentError::ValidationError(_) => StatusCode::BAD_REQUEST,
            GetAttachmentError::NotFound(_) => StatusCode::NOT_FOUND,
            GetAttachmentError::ConflictError(_) => StatusCode::CONFLICT,
            GetAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get attachment metadata",
    skip(pool),
    fields(actor_user_id=%actor_user_id, attachment_id=%attachment_id)
)]
pub async fn get_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    attachment_id: web::Path<Uuid>,
) -> Result<HttpResponse, GetAttachmentError> {
    let attachment_id: AttachmentId = attachment_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::AttachmentAssignment,
        Action::Read(attachment_id.into()),
    )
    .await?
    {
        return Err(GetAttachmentError::ValidationError(
            "You do not have permission to view this attachment".into(),
        ));
    }
    let row = fetch_attachment(&pool, attachment_id)
        .await?
        .ok_or_else(|| GetAttachmentError::NotFound("Attachment not found".into()))?;

    Ok(HttpResponse::Ok().json(AttachmentResponse::from(row)))
}

async fn fetch_attachment(
    pool: &PgPool,
    attachment_id: AttachmentId,
) -> Result<Option<AttachmentRow>, GetAttachmentError> {
    sqlx::query_as!(
        AttachmentRow,
        r#"
            SELECT
                assignments.attachment_id,
                assignments.laboratory_id,
                assignments.file_id,
                assignments.asset_id,
                assignments.inventory_item_id,
                assignments.display_name,
                assignments.description,
                assignments.is_public,
                assignments.assigned_by_user_id,
                assignments.created_at,
                assignments.updated_at,
                files.storage_backend,
                files.storage_key,
                files.original_file_name,
                files.mime_type,
                files.file_size_bytes,
                files.sha256_hex,
                files.uploaded_by_user_id,
                files.created_at AS file_created_at
            FROM asset_attachment_assignments AS assignments
            JOIN files ON files.file_id = assignments.file_id
            WHERE assignments.attachment_id = $1
            "#,
        Uuid::from(attachment_id)
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| GetAttachmentError::UnexpectedError(e.into()))
}
