use super::model::{
    AttachmentResponse, AttachmentRow, AttachmentTarget, create_attachment_rollback_details,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetId, AttachmentDisplayName, FileUploadId, InventoryItemId, LaboratoryId, NewAttachment,
    UserId,
};
use crate::routes::file_uploads::{ConsumeFileUploadError, consume_file_upload};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

/// Request payload describing one upload to be turned into an attachment.
///
/// Shared with the asset / inventory item create endpoints, which accept the same
/// shape inline alongside the entity they create.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentJsonData {
    upload_id: Uuid,
    display_name: Option<String>,
    description: Option<String>,
    is_public: Option<bool>,
}

impl AttachmentJsonData {
    pub(crate) fn upload_id(&self) -> FileUploadId {
        self.upload_id.into()
    }
}

impl TryFrom<AttachmentJsonData> for NewAttachment {
    type Error = String;

    fn try_from(value: AttachmentJsonData) -> Result<Self, Self::Error> {
        Ok(Self::new(
            FileUploadId::parse(value.upload_id)?,
            value
                .display_name
                .map(AttachmentDisplayName::parse)
                .transpose()?,
            value.description,
            value.is_public,
        ))
    }
}

#[derive(thiserror::Error)]
pub enum AssignAttachmentError {
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

impl std::fmt::Debug for AssignAttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for AssignAttachmentError {
    fn status_code(&self) -> StatusCode {
        match self {
            AssignAttachmentError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AssignAttachmentError::Forbidden(_) => StatusCode::FORBIDDEN,
            AssignAttachmentError::NotFound(_) => StatusCode::NOT_FOUND,
            AssignAttachmentError::ConflictError(_) => StatusCode::CONFLICT,
            AssignAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<ConsumeFileUploadError> for AssignAttachmentError {
    fn from(error: ConsumeFileUploadError) -> Self {
        match error {
            ConsumeFileUploadError::ValidationError(message) => Self::ValidationError(message),
            ConsumeFileUploadError::NotFound(message) => Self::NotFound(message),
            ConsumeFileUploadError::ConflictError(message) => Self::ConflictError(message),
            ConsumeFileUploadError::UnexpectedError(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Create an asset attachment",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, asset_id=%asset_id)
)]
pub async fn assign_asset_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<Uuid>,
    payload: web::Json<AttachmentJsonData>,
) -> Result<HttpResponse, AssignAttachmentError> {
    let asset_id: AssetId = asset_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::Asset,
        Action::Update(asset_id.into()),
    )
    .await?
    {
        return Err(AssignAttachmentError::Forbidden(
            "You do not have permission to assign attachments for this asset".into(),
        ));
    }

    let upload_id: FileUploadId = payload.upload_id.into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::FileUpload,
        Action::Assign(upload_id.into()),
    )
    .await?
    {
        return Err(AssignAttachmentError::Forbidden(
            "You do not have permission to assign this attachment".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;

    let new_attachment = NewAttachment::try_from(payload.into_inner())
        .map_err(AssignAttachmentError::ValidationError)?;
    let mut rows = assign_uploaded_attachments(
        &mut transaction,
        actor_user_id,
        AttachmentTarget::Asset(asset_id.into()),
        None,
        std::slice::from_ref(&new_attachment),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to create attachment")?;

    Ok(HttpResponse::Created().json(AttachmentResponse::from(rows.remove(0))))
}

#[tracing::instrument(
    name = "Create an inventory item attachment",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn assign_inventory_item_attachment(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
    payload: web::Json<AttachmentJsonData>,
) -> Result<HttpResponse, AssignAttachmentError> {
    let inventory_item_id: InventoryItemId = inventory_item_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::InventoryItem,
        Action::Update(inventory_item_id.into()),
    )
    .await?
    {
        return Err(AssignAttachmentError::Forbidden(
            "You do not have permission to assign attachments for this inventory item".into(),
        ));
    }

    let upload_id: FileUploadId = payload.upload_id.into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::FileUpload,
        Action::Assign(upload_id.into()),
    )
    .await?
    {
        return Err(AssignAttachmentError::Forbidden(
            "You do not have permission to assign this attachment".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;

    let new_attachment = NewAttachment::try_from(payload.into_inner())
        .map_err(AssignAttachmentError::ValidationError)?;
    let mut rows = assign_uploaded_attachments(
        &mut transaction,
        actor_user_id,
        AttachmentTarget::InventoryItem(inventory_item_id.into()),
        None,
        std::slice::from_ref(&new_attachment),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to create attachment")?;

    Ok(HttpResponse::Created().json(AttachmentResponse::from(rows.remove(0))))
}

// #[derive(sqlx::FromRow)]
// struct LaboratoryIdRow {
//     pub laboratory_id: Uuid,
// }

// async fn fetch_asset_laboratory_id_for_update(
//     transaction: &mut Transaction<'_, Postgres>,
//     asset_id: AssetId,
// ) -> Result<LaboratoryId, AssignAttachmentError> {
//     sqlx::query_as!(
//         LaboratoryIdRow,
//         r#"
//         SELECT laboratory_id
//         FROM assets
//         WHERE asset_id = $1
//         FOR UPDATE
//         "#,
//         Uuid::from(asset_id)
//     )
//     .fetch_optional(transaction.as_mut())
//     .await
//     .map_err(|e| AssignAttachmentError::UnexpectedError(e.into()))?
//     .map(|row| row.laboratory_id.into())
//     .ok_or_else(|| AssignAttachmentError::NotFound("Asset not found".into()))
// }

// async fn fetch_inventory_item_laboratory_id_for_update(
//     transaction: &mut Transaction<'_, Postgres>,
//     inventory_item_id: InventoryItemId,
// ) -> Result<LaboratoryId, AssignAttachmentError> {
//     sqlx::query_as!(
//         LaboratoryIdRow,
//         r#"
//         SELECT laboratory_id
//         FROM asset_inventory_items
//         WHERE inventory_item_id = $1
//         FOR UPDATE
//         "#,
//         Uuid::from(inventory_item_id)
//     )
//     .fetch_optional(transaction.as_mut())
//     .await
//     .map_err(|e| AssignAttachmentError::UnexpectedError(e.into()))?
//     .map(|row| row.laboratory_id.into())
//     .ok_or_else(|| AssignAttachmentError::NotFound("Inventory item not found".into()))
// }

/// Consumes the given uploads and turns each into an attachment on `target`.
///
/// `expected_laboratory_id` is checked against every upload when supplied. The
/// create endpoints pass it because they already know the laboratory they are
/// writing into and want an explicit error; the handlers above only hold the
/// target id and pass `None`, relying on the `(asset_id, laboratory_id)` and
/// `(inventory_item_id, laboratory_id)` composite foreign keys to reject uploads
/// from another laboratory.
pub(crate) async fn assign_uploaded_attachments(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: UserId,
    target: AttachmentTarget,
    expected_laboratory_id: Option<LaboratoryId>,
    attachments: &[NewAttachment],
) -> Result<Vec<AttachmentRow>, AssignAttachmentError> {
    if attachments.is_empty() {
        return Ok(Vec::new());
    }
    let mut seen = HashSet::new();
    for attachment in attachments {
        if !seen.insert(*attachment.upload_id) {
            return Err(AssignAttachmentError::ValidationError(
                "attachments cannot contain duplicate upload_id values".into(),
            ));
        }
    }

    let mut rows = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        let upload = consume_file_upload(transaction, attachment.upload_id).await?;
        if let Some(expected_laboratory_id) = expected_laboratory_id {
            if upload.laboratory_id != Uuid::from(expected_laboratory_id) {
                return Err(AssignAttachmentError::ValidationError(
                    "File upload does not belong to the target laboratory".into(),
                ));
            }
        }
        let display_name = match attachment.display_name.clone() {
            Some(value) => value.as_ref().to_string(),
            None => upload.original_file_name.clone(),
        };
        let description = attachment.description.as_ref().map(|value| value.as_str());
        let is_public = attachment.is_public;
        let (asset_id, inventory_item_id) = match &target {
            AttachmentTarget::Asset(asset_id) => (Some(*asset_id), None),
            AttachmentTarget::InventoryItem(inventory_item_id) => (None, Some(*inventory_item_id)),
        };
        let row = sqlx::query_as::<_, AttachmentRow>(
            r#"
                WITH inserted_file AS (
                    INSERT INTO files (
                        file_id,
                        laboratory_id,
                        storage_backend,
                        storage_key,
                        original_file_name,
                        mime_type,
                        file_size_bytes,
                        sha256_hex,
                        uploaded_by_user_id
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    RETURNING
                        file_id,
                        laboratory_id,
                        storage_backend,
                        storage_key,
                        original_file_name,
                        mime_type,
                        file_size_bytes,
                        sha256_hex,
                        uploaded_by_user_id,
                        created_at
                ),
                inserted_assignment AS (
                    INSERT INTO asset_attachment_assignments (
                        attachment_id,
                        laboratory_id,
                        file_id,
                        asset_id,
                        inventory_item_id,
                        display_name,
                        description,
                        is_public,
                        assigned_by_user_id
                    )
                    SELECT
                        $10,
                        inserted_file.laboratory_id,
                        inserted_file.file_id,
                        $11,
                        $12,
                        $13,
                        $14,
                        $15,
                        $16
                    FROM inserted_file
                    RETURNING
                        attachment_id,
                        laboratory_id,
                        file_id,
                        asset_id,
                        inventory_item_id,
                        display_name,
                        description,
                        is_public,
                        assigned_by_user_id,
                        created_at,
                        updated_at
                )
                SELECT
                    inserted_assignment.attachment_id,
                    inserted_assignment.laboratory_id,
                    inserted_assignment.file_id,
                    inserted_assignment.asset_id,
                    inserted_assignment.inventory_item_id,
                    inserted_assignment.display_name,
                    inserted_assignment.description,
                    inserted_assignment.is_public,
                    inserted_assignment.assigned_by_user_id,
                    inserted_assignment.created_at,
                    inserted_assignment.updated_at,
                    inserted_file.storage_backend,
                    inserted_file.storage_key,
                    inserted_file.original_file_name,
                    inserted_file.mime_type,
                    inserted_file.file_size_bytes,
                    inserted_file.sha256_hex,
                    inserted_file.uploaded_by_user_id,
                    inserted_file.created_at AS file_created_at
                FROM inserted_assignment
                JOIN inserted_file
                  ON inserted_file.file_id = inserted_assignment.file_id
                "#,
        )
        .bind(Uuid::new_v4())
        .bind(upload.laboratory_id)
        .bind(&upload.storage_backend)
        .bind(&upload.storage_key)
        .bind(&upload.original_file_name)
        .bind(upload.mime_type.as_deref())
        .bind(upload.file_size_bytes)
        .bind(&upload.sha256_hex)
        .bind(upload.uploaded_by_user_id)
        .bind(Uuid::new_v4())
        .bind(asset_id)
        .bind(inventory_item_id)
        .bind(&display_name)
        .bind(description)
        .bind(is_public)
        .bind(*actor_user_id)
        .fetch_one(transaction.as_mut())
        .await
        .map_err(map_assign_database_error)?;

        record_audit(
            transaction,
            actor_user_id,
            AuditAction::Create,
            AuditResource::Attachment,
            Some(row.attachment_id),
            create_attachment_rollback_details(&row),
        )
        .await?;
        rows.push(row);
    }
    Ok(rows)
}

fn map_assign_database_error(error: sqlx::Error) -> AssignAttachmentError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => {
                return AssignAttachmentError::ConflictError("Attachment already exists".into());
            }
            Some("23503") => {
                return AssignAttachmentError::ValidationError("Invalid referenced record".into());
            }
            Some("23514") => {
                return AssignAttachmentError::ValidationError("Invalid attachment data".into());
            }
            _ => {}
        }
    }
    AssignAttachmentError::UnexpectedError(error.into())
}
