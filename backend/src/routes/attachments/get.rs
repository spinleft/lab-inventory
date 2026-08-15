use super::model::AttachmentResponse;
use super::queries::fetch_attachment;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::AttachmentId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum GetAttachmentError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
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
            GetAttachmentError::Forbidden(_) => StatusCode::FORBIDDEN,
            GetAttachmentError::NotFound(_) => StatusCode::NOT_FOUND,
            GetAttachmentError::ConflictError(_) => StatusCode::CONFLICT,
            GetAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get attachment metadata",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, attachment_id=%attachment_id)
)]
pub async fn get_attachment(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    attachment_id: AttachmentId,
) -> Result<HttpResponse, GetAttachmentError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::AttachmentAssignment,
        Action::Read(attachment_id.into()),
    )
    .await?
    {
        return Err(GetAttachmentError::Forbidden(
            "You do not have permission to view this attachment".into(),
        ));
    }
    let row = fetch_attachment(&pool, attachment_id)
        .await?
        .ok_or_else(|| GetAttachmentError::NotFound("Attachment not found".into()))?;

    Ok(HttpResponse::Ok().json(AttachmentResponse::from(row)))
}
