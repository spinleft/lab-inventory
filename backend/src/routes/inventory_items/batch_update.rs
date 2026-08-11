use super::model::{InventoryItemResponse, update_inventory_item_rollback_details};
use super::queries::{InventoryItemDatabaseError, fetch_inventory_items_for_update};
use super::service::{apply_inventory_item_patch, validate_requested_ids};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{InventoryItemIds, UpdateInventoryItem};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    inventory_item_ids: Vec<Uuid>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    batch_number: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    location_id: Option<Option<Uuid>>,
    status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    public_notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    internal_notes: Option<Option<String>>,
}

impl TryFrom<&JsonData> for UpdateInventoryItem {
    type Error = String;

    fn try_from(value: &JsonData) -> Result<Self, Self::Error> {
        Self::parse(
            None,
            value.batch_number.clone(),
            None,
            None,
            value
                .location_id
                .map(|location_id| location_id.map(Uuid::into)),
            value.status.clone(),
            value.public_notes.clone(),
            value.internal_notes.clone(),
        )
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
pub enum BatchUpdateInventoryItemsError {
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

impl std::fmt::Debug for BatchUpdateInventoryItemsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for BatchUpdateInventoryItemsError {
    fn status_code(&self) -> StatusCode {
        match self {
            BatchUpdateInventoryItemsError::ValidationError(_) => StatusCode::BAD_REQUEST,
            BatchUpdateInventoryItemsError::Forbidden(_) => StatusCode::FORBIDDEN,
            BatchUpdateInventoryItemsError::NotFound(_) => StatusCode::NOT_FOUND,
            BatchUpdateInventoryItemsError::ConflictError(_) => StatusCode::CONFLICT,
            BatchUpdateInventoryItemsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<InventoryItemDatabaseError> for BatchUpdateInventoryItemsError {
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
    name = "Batch update inventory items",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id)
)]
pub async fn batch_update_inventory_items(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, BatchUpdateInventoryItemsError> {
    let actor = laboratory_context.authorization_actor();
    let payload = payload.into_inner();
    let inventory_item_ids = InventoryItemIds::parse(payload.inventory_item_ids.clone())
        .map_err(BatchUpdateInventoryItemsError::ValidationError)?;
    let inventory_item_ids: Vec<_> = inventory_item_ids
        .into_inner()
        .into_iter()
        .map(Uuid::from)
        .collect();
    let patch = UpdateInventoryItem::try_from(&payload)
        .map_err(BatchUpdateInventoryItemsError::ValidationError)?;
    if !patch.has_batch_updates() {
        return Err(BatchUpdateInventoryItemsError::ValidationError(
            "Batch update requires at least one update field".into(),
        ));
    }
    for inventory_item_id in &inventory_item_ids {
        if !validate_permission(
            &pool,
            &actor,
            ResourceType::InventoryItem,
            Action::Update(*inventory_item_id),
        )
        .await?
        {
            return Err(BatchUpdateInventoryItemsError::Forbidden(
                "You don't have permission to update these inventory items.".into(),
            ));
        }
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing_items =
        fetch_inventory_items_for_update(&mut transaction, &inventory_item_ids).await?;
    validate_requested_ids(&inventory_item_ids, &existing_items)?;

    let mut updated_items = Vec::with_capacity(existing_items.len());
    for existing in existing_items {
        let updated =
            apply_inventory_item_patch(&mut transaction, &existing, patch.clone()).await?;
        record_audit(
            &mut transaction,
            laboratory_context.actor(),
            AuditAction::Update,
            AuditResource::InventoryItem,
            Some(updated.inventory_item_id),
            update_inventory_item_rollback_details(&existing),
        )
        .await?;
        updated_items.push(updated);
    }
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to batch update inventory items.")?;

    Ok(HttpResponse::Ok().json(
        updated_items
            .into_iter()
            .map(|item| InventoryItemResponse::from_row(item, true))
            .collect::<Vec<_>>(),
    ))
}
