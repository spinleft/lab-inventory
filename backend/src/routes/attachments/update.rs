use super::model::{
    AttachmentResponse, AttachmentRow, fetch_attachment_for_update,
    update_attachment_rollback_details,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AttachmentDisplayName, AttachmentId, NullableUpdate, UpdateAttachment, UserId,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

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
        Ok(Self::new(display_name, description, value.is_public))
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
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
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
            UpdateAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Update attachment metadata",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, attachment_id=%attachment_id)
)]
pub async fn update_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    attachment_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateAttachmentError> {
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
        &actor_user_id,
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

    let updated = update_attachment_assignment_in_database(
        &mut transaction,
        attachment_id,
        &display_name,
        description.as_deref(),
        is_public,
    )
    .await?;

    record_audit(
        &mut transaction,
        actor_user_id,
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

#[tracing::instrument(
    name = "Updating asset category in the database",
    skip(transaction, display_name, description, is_public),
    fields(attachment_id=%attachment_id)
)]
async fn update_attachment_assignment_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    attachment_id: AttachmentId,
    display_name: &str,
    description: Option<&str>,
    is_public: bool,
) -> Result<AttachmentRow, UpdateAttachmentError> {
    sqlx::query_as!(
        AttachmentRow,
        r#"
            WITH updated_assignment AS (
                UPDATE asset_attachment_assignments
                SET
                    display_name = $2,
                    description = $3,
                    is_public = $4,
                    updated_at = now()
                WHERE attachment_id = $1
                RETURNING *
            )
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
            FROM updated_assignment AS assignments
            JOIN files ON files.file_id = assignments.file_id
            "#,
        Uuid::from(attachment_id),
        display_name,
        description,
        is_public,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(|e| UpdateAttachmentError::UnexpectedError(e.into()))
}
