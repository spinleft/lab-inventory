use super::helpers::{
    InventoryTransactionData, ensure_can_write, fetch_inventory_item, map_database_error,
    record_inventory_transaction, validate_positive_quantity,
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
    quantity: f64,
}

#[tracing::instrument(name = "Release inventory item allocation", skip(request, pool, payload), fields(user_id=%user_id, inventory_item_id=%inventory_item_id))]
pub async fn release_inventory_item_allocation(
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
    validate_positive_quantity(payload.quantity, "quantity", existing.unit_allow_decimal)?;
    if existing.tracking_mode == "serialized" && payload.quantity != 1.0 {
        return Err(ApiError::BadRequest(
            "serialized inventory release quantity must be 1".into(),
        ));
    }
    if payload.quantity > existing.quantity_allocated {
        return Err(ApiError::BadRequest(
            "release quantity cannot exceed quantity_allocated".into(),
        ));
    }
    let new_allocated = existing.quantity_allocated - payload.quantity;

    match try_processing(pool.get_ref(), &idempotency_key, *user_id).await? {
        NextAction::ReturnSavedResponse(response) => Ok(response),
        NextAction::StartProcessing(mut transaction) => {
            let item = update_allocated(&mut transaction, inventory_item_id, new_allocated).await?;
            record_inventory_transaction(
                &mut transaction,
                &actor,
                InventoryTransactionData {
                    inventory_item_id: Some(item.inventory_item_id),
                    laboratory_id: item.laboratory_id,
                    action: AuditAction::ReleaseAllocation,
                    quantity_delta: 0.0,
                    allocated_delta: -payload.quantity,
                    from_location_id: item.location_id,
                    to_location_id: item.location_id,
                    related_resource_type: None,
                    related_resource_id: None,
                    details: json!({ "quantity_allocated": item.quantity_allocated }),
                },
            )
            .await?;
            record_audit(
                &mut transaction,
                &actor,
                Some(item.laboratory_id),
                AuditAction::ReleaseAllocation,
                AuditResource::InventoryItem,
                Some(item.inventory_item_id),
                json!({ "quantity": payload.quantity }),
            )
            .await?;
            let response = HttpResponse::Ok().json(InventoryItemResponse::from_row(item, &actor));
            save_response(transaction, &idempotency_key, *user_id, response).await
        }
    }
}

async fn update_allocated(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    inventory_item_id: Uuid,
    quantity_allocated: f64,
) -> Result<InventoryItemRow, ApiError> {
    sqlx::query_as::<_, InventoryItemRow>(
        r#"
        UPDATE asset_inventory_items
        SET quantity_allocated = $2, updated_at = now()
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
    .bind(quantity_allocated)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}
