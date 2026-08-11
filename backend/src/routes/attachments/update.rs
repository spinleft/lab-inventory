use super::model::{AttachmentResponse, update_attachment_rollback_details};
use super::queries::{
    AttachmentDatabaseError, fetch_attachment_for_update, update_attachment_in_database,
};
use crate::access_control::AttachmentPathId;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AttachmentDisplayName, AttachmentId, NullableUpdate, UpdateAttachment};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    pub display_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub description: Option<Option<String>>,
    pub is_public: Option<bool>,
}

impl TryFrom<JsonData> for UpdateAttachment {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let display_name = value
            .display_name
            .map(AttachmentDisplayName::parse)
            .transpose()?;
        let description = match value.description {
            Some(Some(description)) => NullableUpdate::Set(description),
            Some(None) => NullableUpdate::Clear,
            None => NullableUpdate::Unchanged,
        };
        Ok(Self {
            display_name,
            description,
            is_public: value.is_public,
        })
    }
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(thiserror::Error)]
pub enum UpdateAttachmentError {
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

impl From<AttachmentDatabaseError> for UpdateAttachmentError {
    fn from(error: AttachmentDatabaseError) -> Self {
        match error {
            AttachmentDatabaseError::Validation(message) => Self::ValidationError(message),
            AttachmentDatabaseError::Conflict(message) => Self::ConflictError(message),
            AttachmentDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

impl std::fmt::Debug for UpdateAttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UpdateAttachmentError {
    fn status_code(&self) -> StatusCode {
        match self {
            UpdateAttachmentError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UpdateAttachmentError::Forbidden(_) => StatusCode::FORBIDDEN,
            UpdateAttachmentError::NotFound(_) => StatusCode::NOT_FOUND,
            UpdateAttachmentError::ConflictError(_) => StatusCode::CONFLICT,
            UpdateAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Update attachment metadata",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, attachment_id=%attachment_id)
)]
pub async fn update_attachment(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    attachment_id: AttachmentPathId,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateAttachmentError> {
    let actor = laboratory_context.authorization_actor();
    let attachment_id: AttachmentId = attachment_id.into_inner().into();
    let update_attachment = UpdateAttachment::try_from(payload.into_inner())
        .map_err(UpdateAttachmentError::ValidationError)?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_attachment_for_update(&mut transaction, attachment_id)
        .await?
        .ok_or_else(|| UpdateAttachmentError::NotFound("Attachment not found".into()))?;
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::AttachmentAssignment,
        Action::Update(attachment_id.into()),
    )
    .await?
    {
        return Err(UpdateAttachmentError::Forbidden(
            "You do not have permission to update this attachment".into(),
        ));
    }

    let display_name = update_attachment
        .display_name
        .as_ref()
        .map(|value| value.as_ref())
        .unwrap_or(&existing.display_name)
        .to_string();
    let description = update_attachment
        .description
        .resolve(existing.description.clone());
    let is_public = update_attachment.is_public.unwrap_or(existing.is_public);

    let updated = update_attachment_in_database(
        &mut transaction,
        attachment_id,
        &display_name,
        description.as_deref(),
        is_public,
    )
    .await?;

    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Update,
        AuditResource::Attachment,
        Some(updated.attachment_id),
        update_attachment_rollback_details(&existing),
    )
    .await?;

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update attachment")?;

    Ok(HttpResponse::Ok().json(AttachmentResponse::from(updated)))
}
