use super::model::{
    InventoryItemDatabaseError, InventoryItemResponse, apply_inventory_item_patch,
    fetch_inventory_item_for_update, update_inventory_item_rollback_details,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{InventoryItemId, UpdateInventoryItem, UserId};
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
    serial_number: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    batch_number: Option<Option<String>>,
    quantity_on_hand: Option<f64>,
    quantity_allocated: Option<f64>,
    quantity_unit_id: Option<Uuid>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    location_id: Option<Option<Uuid>>,
    status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    public_notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    internal_notes: Option<Option<String>>,
}

impl TryFrom<JsonData> for UpdateInventoryItem {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        Self::parse(
            value.serial_number,
            value.batch_number,
            value.quantity_on_hand,
            value.quantity_allocated,
            value.quantity_unit_id.map(Uuid::into),
            value
                .location_id
                .map(|location_id| location_id.map(Uuid::into)),
            value.status,
            value.public_notes,
            value.internal_notes,
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
pub enum UpdateInventoryItemError {
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

impl std::fmt::Debug for UpdateInventoryItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UpdateInventoryItemError {
    fn status_code(&self) -> StatusCode {
        match self {
            UpdateInventoryItemError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UpdateInventoryItemError::Forbidden(_) => StatusCode::FORBIDDEN,
            UpdateInventoryItemError::NotFound(_) => StatusCode::NOT_FOUND,
            UpdateInventoryItemError::ConflictError(_) => StatusCode::CONFLICT,
            UpdateInventoryItemError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<InventoryItemDatabaseError> for UpdateInventoryItemError {
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
    name = "Update an inventory item",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn update_inventory_item(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateInventoryItemError> {
    let inventory_item_id: InventoryItemId = inventory_item_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::InventoryItem,
        Action::Update(inventory_item_id.into()),
    )
    .await?
    {
        return Err(UpdateInventoryItemError::Forbidden(
            "You don't have permission to update this inventory item.".into(),
        ));
    }

    let patch = UpdateInventoryItem::try_from(payload.into_inner())
        .map_err(UpdateInventoryItemError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_inventory_item_for_update(&mut transaction, inventory_item_id.into())
        .await?
        .ok_or(UpdateInventoryItemError::NotFound(
            "Inventory item not found".into(),
        ))?;
    let updated = apply_inventory_item_patch(&mut transaction, &existing, patch).await?;

    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Update,
        AuditResource::InventoryItem,
        Some(updated.inventory_item_id),
        update_inventory_item_rollback_details(&existing),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update an inventory item.")?;

    Ok(HttpResponse::Ok().json(InventoryItemResponse::from_row(updated, true)))
}
