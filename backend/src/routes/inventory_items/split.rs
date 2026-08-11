use super::model::{InventoryItemResponse, split_inventory_item_rollback_details};
use super::queries::{
    InventoryItemDatabaseError, fetch_inventory_item_for_update,
    find_quantity_aggregate_for_update, validate_location,
};
use super::service::{
    add_quantities_to_item, insert_inventory_item, set_quantity_on_hand, validate_quantity_item,
};
use crate::access_control::InventoryItemPathId;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{InventoryItemId, SplitInventoryItem as SplitInventoryItemCommand};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    quantity: f64,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    batch_number: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    location_id: Option<Option<Uuid>>,
    status: Option<String>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
}

#[derive(Serialize)]
struct SplitInventoryItemResponse {
    source: InventoryItemResponse,
    target: InventoryItemResponse,
}

impl TryFrom<JsonData> for SplitInventoryItemCommand {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        Self::parse(
            value.quantity,
            value.batch_number,
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
pub enum SplitInventoryItemError {
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

impl std::fmt::Debug for SplitInventoryItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SplitInventoryItemError {
    fn status_code(&self) -> StatusCode {
        match self {
            SplitInventoryItemError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SplitInventoryItemError::Forbidden(_) => StatusCode::FORBIDDEN,
            SplitInventoryItemError::NotFound(_) => StatusCode::NOT_FOUND,
            SplitInventoryItemError::ConflictError(_) => StatusCode::CONFLICT,
            SplitInventoryItemError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<InventoryItemDatabaseError> for SplitInventoryItemError {
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
    name = "Split an inventory item",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn split_inventory_item(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    inventory_item_id: InventoryItemPathId,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, SplitInventoryItemError> {
    let actor = laboratory_context.authorization_actor();
    let inventory_item_id: InventoryItemId = inventory_item_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::InventoryItem,
        Action::Update(inventory_item_id.into()),
    )
    .await?
    {
        return Err(SplitInventoryItemError::Forbidden(
            "You are not allowed to split this inventory item.".into(),
        ));
    }

    let command = SplitInventoryItemCommand::try_from(payload.into_inner())
        .map_err(SplitInventoryItemError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let source_before = fetch_inventory_item_for_update(&mut transaction, inventory_item_id.into())
        .await?
        .ok_or(SplitInventoryItemError::NotFound(
            "Inventory item not found".into(),
        ))?;
    validate_quantity_item(&source_before).map_err(SplitInventoryItemError::ValidationError)?;
    if command.quantity > source_before.quantity_on_hand - source_before.quantity_allocated {
        return Err(SplitInventoryItemError::ValidationError(
            "Split quantity cannot exceed unallocated quantity".into(),
        ));
    }

    let target_status = command
        .status
        .map(|status| status.as_str().to_string())
        .unwrap_or_else(|| source_before.status.clone());
    let target_batch_number = command
        .batch_number
        .resolve(source_before.batch_number.clone());
    let target_location_id = command
        .location_id
        .resolve(source_before.location_id.map(Uuid::into))
        .map(Uuid::from);
    if let Some(location_id) = target_location_id {
        validate_location(&mut transaction, source_before.laboratory_id, location_id).await?;
    }
    if target_batch_number == source_before.batch_number
        && target_location_id == source_before.location_id
        && target_status == source_before.status
    {
        return Err(SplitInventoryItemError::ValidationError(
            "Split target must differ by batch, location, or status".into(),
        ));
    }

    let target_before = find_quantity_aggregate_for_update(
        &mut transaction,
        source_before.laboratory_id,
        source_before.asset_id,
        target_batch_number.as_deref(),
        target_location_id,
        &target_status,
        Some(source_before.inventory_item_id),
    )
    .await?;
    let source_after = set_quantity_on_hand(
        &mut transaction,
        source_before.inventory_item_id,
        source_before.quantity_on_hand - command.quantity,
    )
    .await?;
    let target_after = match target_before.as_ref() {
        Some(target) => {
            add_quantities_to_item(
                &mut transaction,
                target.inventory_item_id,
                command.quantity,
                0.0,
            )
            .await?
        }
        None => {
            insert_inventory_item(
                &mut transaction,
                source_before.asset_id,
                source_before.laboratory_id,
                "quantity",
                None,
                target_batch_number.as_deref(),
                command.quantity,
                0.0,
                target_location_id,
                &target_status,
                command
                    .public_notes
                    .as_deref()
                    .or(source_before.public_notes.as_deref()),
                command
                    .internal_notes
                    .as_deref()
                    .or(source_before.internal_notes.as_deref()),
            )
            .await?
        }
    };

    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Adjust,
        AuditResource::InventoryItem,
        Some(source_after.inventory_item_id),
        split_inventory_item_rollback_details(
            &source_before,
            target_before.as_ref(),
            &target_after,
        ),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to split an inventory item.")?;

    Ok(HttpResponse::Ok().json(SplitInventoryItemResponse {
        source: InventoryItemResponse::from_row(source_after, true),
        target: InventoryItemResponse::from_row(target_after, true),
    }))
}
