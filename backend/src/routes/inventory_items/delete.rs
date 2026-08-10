use super::model::{DeletedAttachmentRow, delete_inventory_item_rollback_details};
use super::queries::{
    InventoryItemDatabaseError, delete_inventory_item_attachments,
    delete_inventory_item_from_database, fetch_inventory_item_for_update,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{FileStorageKey, InventoryItemId, UserId};
use crate::file_storage::FileStorage;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum DeleteInventoryItemError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for DeleteInventoryItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteInventoryItemError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteInventoryItemError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteInventoryItemError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteInventoryItemError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteInventoryItemError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<InventoryItemDatabaseError> for DeleteInventoryItemError {
    fn from(error: InventoryItemDatabaseError) -> Self {
        match error {
            InventoryItemDatabaseError::Validation(message) => Self::ConflictError(message),
            InventoryItemDatabaseError::NotFound(message) => Self::NotFound(message),
            InventoryItemDatabaseError::Conflict(message) => Self::ConflictError(message),
            InventoryItemDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Delete an inventory item",
    skip(pool, storage),
    fields(actor_user_id=%actor_user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn delete_inventory_item(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<FileStorage>,
    inventory_item_id: web::Path<Uuid>,
) -> Result<HttpResponse, DeleteInventoryItemError> {
    let inventory_item_id: InventoryItemId = inventory_item_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::InventoryItem,
        Action::Delete(inventory_item_id.into()),
    )
    .await?
    {
        return Err(DeleteInventoryItemError::Forbidden(
            "You don't have permission to delete this inventory item.".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let item = fetch_inventory_item_for_update(&mut transaction, inventory_item_id.into())
        .await?
        .ok_or(DeleteInventoryItemError::NotFound(
            "Inventory item not found".into(),
        ))?;
    if item.quantity_allocated > 0.0 {
        return Err(DeleteInventoryItemError::ConflictError(
            "Cannot delete inventory items with allocated quantity".into(),
        ));
    }

    let deleted_attachments =
        delete_inventory_item_attachments(&mut transaction, item.inventory_item_id).await?;
    let attachment_ids: Vec<_> = deleted_attachments
        .iter()
        .map(|attachment| attachment.attachment_id)
        .collect();
    delete_inventory_item_from_database(&mut transaction, item.inventory_item_id).await?;

    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Delete,
        AuditResource::InventoryItem,
        Some(item.inventory_item_id),
        delete_inventory_item_rollback_details(&item, &attachment_ids),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete an inventory item.")?;
    delete_storage_objects(&storage, &deleted_attachments).await?;

    Ok(HttpResponse::NoContent().finish())
}

pub(super) async fn delete_storage_objects(
    storage: &FileStorage,
    attachments: &[DeletedAttachmentRow],
) -> Result<(), anyhow::Error> {
    for attachment in attachments {
        let storage_key =
            FileStorageKey::parse(attachment.storage_key.clone()).map_err(anyhow::Error::msg)?;
        storage.delete(&storage_key).await?;
    }

    Ok(())
}
