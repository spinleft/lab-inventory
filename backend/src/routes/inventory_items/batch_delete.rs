use super::delete::delete_storage_objects;
use super::model::delete_inventory_item_rollback_details;
use super::queries::{
    InventoryItemDatabaseError, delete_inventory_item_attachments,
    delete_inventory_item_from_database, fetch_inventory_items_for_update,
};
use super::service::validate_requested_ids;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::InventoryItemIds;
use crate::file_storage::FileStorage;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    inventory_item_ids: Vec<Uuid>,
}

#[derive(thiserror::Error)]
pub enum BatchDeleteInventoryItemsError {
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

impl std::fmt::Debug for BatchDeleteInventoryItemsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for BatchDeleteInventoryItemsError {
    fn status_code(&self) -> StatusCode {
        match self {
            BatchDeleteInventoryItemsError::ValidationError(_) => StatusCode::BAD_REQUEST,
            BatchDeleteInventoryItemsError::Forbidden(_) => StatusCode::FORBIDDEN,
            BatchDeleteInventoryItemsError::NotFound(_) => StatusCode::NOT_FOUND,
            BatchDeleteInventoryItemsError::ConflictError(_) => StatusCode::CONFLICT,
            BatchDeleteInventoryItemsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<InventoryItemDatabaseError> for BatchDeleteInventoryItemsError {
    fn from(error: InventoryItemDatabaseError) -> Self {
        match error {
            InventoryItemDatabaseError::Validation(message) => Self::ValidationError(message),
            InventoryItemDatabaseError::NotFound(message) => Self::NotFound(message),
            InventoryItemDatabaseError::Conflict(message) => Self::ConflictError(message),
            InventoryItemDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Batch delete inventory items",
    skip(pool, storage, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id)
)]
pub async fn batch_delete_inventory_items(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    storage: web::Data<FileStorage>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, BatchDeleteInventoryItemsError> {
    let actor = laboratory_context.authorization_actor();
    let inventory_item_ids = InventoryItemIds::parse(payload.into_inner().inventory_item_ids)
        .map_err(BatchDeleteInventoryItemsError::ValidationError)?;
    let inventory_item_ids: Vec<_> = inventory_item_ids
        .into_inner()
        .into_iter()
        .map(Uuid::from)
        .collect();
    for inventory_item_id in &inventory_item_ids {
        if !validate_permission(
            &pool,
            &actor,
            ResourceType::InventoryItem,
            Action::Delete(*inventory_item_id),
        )
        .await?
        {
            return Err(BatchDeleteInventoryItemsError::Forbidden(
                "You don't have permission to delete these inventory items.".into(),
            ));
        }
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let items = fetch_inventory_items_for_update(&mut transaction, &inventory_item_ids).await?;
    validate_requested_ids(&inventory_item_ids, &items)?;
    for item in &items {
        if item.quantity_allocated > 0.0 {
            return Err(BatchDeleteInventoryItemsError::ConflictError(
                "Cannot delete inventory items with allocated quantity".into(),
            ));
        }
    }

    let mut deleted_attachments = Vec::new();
    for item in items {
        let item_attachments =
            delete_inventory_item_attachments(&mut transaction, item.inventory_item_id).await?;
        let attachment_ids: Vec<_> = item_attachments
            .iter()
            .map(|attachment| attachment.attachment_id)
            .collect();
        deleted_attachments.extend(item_attachments);
        delete_inventory_item_from_database(&mut transaction, item.inventory_item_id).await?;
        record_audit(
            &mut transaction,
            laboratory_context.actor(),
            AuditAction::Delete,
            AuditResource::InventoryItem,
            Some(item.inventory_item_id),
            delete_inventory_item_rollback_details(&item, &attachment_ids),
        )
        .await?;
    }
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to batch delete inventory items.")?;
    delete_storage_objects(&storage, &deleted_attachments).await?;

    Ok(HttpResponse::NoContent().finish())
}
