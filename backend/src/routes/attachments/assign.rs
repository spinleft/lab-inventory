use super::model::{AttachmentResponse, AttachmentTarget};
use super::queries::{fetch_asset_laboratory_id, fetch_inventory_item_laboratory_id};
use super::service::{AssignAttachmentError, assign_uploaded_attachments};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::{AssetId, AttachmentDisplayName, FileUploadId, InventoryItemId, NewAttachment};
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
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

#[tracing::instrument(
    name = "Create an asset attachment",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, asset_id=%asset_id)
)]
pub async fn assign_asset_attachment(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    asset_id: AssetId,
    payload: web::Json<AttachmentJsonData>,
) -> Result<HttpResponse, AssignAttachmentError> {
    let actor = laboratory_context.authorization_actor();
    let asset_laboratory_id = fetch_asset_laboratory_id(&pool, asset_id)
        .await?
        .ok_or_else(|| AssignAttachmentError::NotFound("Asset not found".into()))?;
    if laboratory_context.laboratory_id() != asset_laboratory_id {
        return Err(AssignAttachmentError::NotFound("Asset not found".into()));
    }
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::Asset,
        Action::Update(asset_id.into()),
    )
    .await?
    {
        return Err(AssignAttachmentError::Forbidden(
            "You do not have permission to assign attachments for this asset".into(),
        ));
    }

    let upload_id = payload.upload_id();
    if !validate_permission(
        &pool,
        &actor,
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
        laboratory_context.actor().user_id,
        AttachmentTarget::Asset(asset_id.into()),
        Some(asset_laboratory_id),
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
    fields(actor_user_id=%laboratory_context.actor().user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn assign_inventory_item_attachment(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    inventory_item_id: InventoryItemId,
    payload: web::Json<AttachmentJsonData>,
) -> Result<HttpResponse, AssignAttachmentError> {
    let actor = laboratory_context.authorization_actor();
    let inventory_item_laboratory_id = fetch_inventory_item_laboratory_id(&pool, inventory_item_id)
        .await?
        .ok_or_else(|| AssignAttachmentError::NotFound("Inventory item not found".into()))?;
    if laboratory_context.laboratory_id() != inventory_item_laboratory_id {
        return Err(AssignAttachmentError::NotFound(
            "Inventory item not found".into(),
        ));
    }
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::InventoryItem,
        Action::Update(inventory_item_id.into()),
    )
    .await?
    {
        return Err(AssignAttachmentError::Forbidden(
            "You do not have permission to assign attachments for this inventory item".into(),
        ));
    }

    let upload_id = payload.upload_id();
    if !validate_permission(
        &pool,
        &actor,
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
        laboratory_context.actor().user_id,
        AttachmentTarget::InventoryItem(inventory_item_id.into()),
        Some(inventory_item_laboratory_id),
        std::slice::from_ref(&new_attachment),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to create attachment")?;

    Ok(HttpResponse::Created().json(AttachmentResponse::from(rows.remove(0))))
}
