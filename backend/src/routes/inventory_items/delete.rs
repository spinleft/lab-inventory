use super::helpers::{
    InventoryTransactionData, ensure_can_write, fetch_inventory_item, map_database_error,
    record_inventory_transaction,
};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Delete an inventory item", skip(pool), fields(user_id=%user_id, inventory_item_id=%inventory_item_id))]
pub async fn delete_inventory_item(
    user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let inventory_item_id = inventory_item_id.into_inner();
    let item = fetch_inventory_item(pool.get_ref(), inventory_item_id).await?;
    ensure_can_write(&actor, item.laboratory_id)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    record_inventory_transaction(
        &mut transaction,
        &actor,
        InventoryTransactionData {
            inventory_item_id: Some(item.inventory_item_id),
            laboratory_id: item.laboratory_id,
            action: AuditAction::Delete,
            quantity_delta: -item.quantity_on_hand,
            allocated_delta: -item.quantity_allocated,
            from_location_id: item.location_id,
            to_location_id: None,
            related_resource_type: None,
            related_resource_id: None,
            details: json!({ "asset_id": item.asset_id, "serial_number": item.serial_number }),
        },
    )
    .await?;
    sqlx::query("DELETE FROM asset_inventory_items WHERE inventory_item_id = $1")
        .bind(inventory_item_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
    record_audit(
        &mut transaction,
        &actor,
        Some(item.laboratory_id),
        AuditAction::Delete,
        AuditResource::InventoryItem,
        Some(item.inventory_item_id),
        json!({ "asset_id": item.asset_id, "serial_number": item.serial_number }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::NoContent().finish())
}
