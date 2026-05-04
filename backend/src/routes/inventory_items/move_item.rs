use super::helpers::{
    InventoryTransactionData, ensure_can_write, fetch_inventory_item, map_database_error,
    record_inventory_transaction, validate_location,
};
use super::model::{InventoryItemResponse, InventoryItemRow};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::idempotency::{NextAction, idempotency_key_from_request, save_response, try_processing};
use crate::utils::ApiError;
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    location_id: Uuid,
}

#[tracing::instrument(name = "Move an inventory item", skip(request, pool, payload), fields(user_id=%user_id, inventory_item_id=%inventory_item_id))]
pub async fn move_inventory_item(
    request: HttpRequest,
    user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let idempotency_key = idempotency_key_from_request(&request)?;
    let inventory_item_id = inventory_item_id.into_inner();
    let existing = fetch_inventory_item(pool.get_ref(), inventory_item_id).await?;
    ensure_can_write(&actor, existing.laboratory_id)?;
    validate_location(
        pool.get_ref(),
        existing.laboratory_id,
        Some(payload.location_id),
    )
    .await?;

    match try_processing(pool.get_ref(), &idempotency_key, *user_id).await? {
        NextAction::ReturnSavedResponse(response) => Ok(response),
        NextAction::StartProcessing(mut transaction) => {
            let item = sqlx::query_as::<_, InventoryItemRow>(
                r#"
                UPDATE asset_inventory_items
                SET location_id = $2, updated_at = now()
                WHERE inventory_item_id = $1
                RETURNING
                    inventory_item_id,
                    asset_id,
                    (SELECT name FROM assets WHERE asset_id = asset_inventory_items.asset_id) AS asset_name,
                    (SELECT model FROM assets WHERE asset_id = asset_inventory_items.asset_id) AS asset_model,
                    laboratory_id,
                    (SELECT name FROM laboratories WHERE laboratory_id = asset_inventory_items.laboratory_id) AS laboratory_name,
                    tracking_mode,
                    serial_number,
                    batch_number,
                    quantity_on_hand,
                    quantity_allocated,
                    unit_id,
                    (SELECT code FROM units WHERE unit_id = asset_inventory_items.unit_id) AS unit_code,
                    (SELECT allow_decimal FROM units WHERE unit_id = asset_inventory_items.unit_id) AS unit_allow_decimal,
                    location_id,
                    (SELECT name FROM locations WHERE location_id = asset_inventory_items.location_id) AS location_name,
                    status,
                    is_cross_lab_borrowable,
                    public_notes,
                    internal_notes,
                    created_at,
                    updated_at
                "#,
            )
            .bind(inventory_item_id)
            .bind(payload.location_id)
            .fetch_one(transaction.as_mut())
            .await
            .map_err(map_database_error)?;
            record_inventory_transaction(
                &mut transaction,
                &actor,
                InventoryTransactionData {
                    inventory_item_id: Some(item.inventory_item_id),
                    laboratory_id: item.laboratory_id,
                    action: AuditAction::Move,
                    quantity_delta: 0.0,
                    allocated_delta: 0.0,
                    from_location_id: existing.location_id,
                    to_location_id: item.location_id,
                    related_resource_type: None,
                    related_resource_id: None,
                    details: json!({ "to_location_id": item.location_id }),
                },
            )
            .await?;
            record_audit(
                &mut transaction,
                &actor,
                Some(item.laboratory_id),
                AuditAction::Move,
                AuditResource::InventoryItem,
                Some(item.inventory_item_id),
                json!({ "to_location_id": item.location_id }),
            )
            .await?;
            let response = HttpResponse::Ok().json(InventoryItemResponse::from_row(item, &actor));
            save_response(transaction, &idempotency_key, *user_id, response).await
        }
    }
}
