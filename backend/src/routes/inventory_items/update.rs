use super::helpers::{
    InventoryTransactionData, ensure_can_write, fetch_inventory_item, map_database_error,
    record_inventory_transaction, validate_location, validate_status,
};
use super::model::{InventoryItemResponse, InventoryItemRow};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    batch_number: Option<String>,
    location_id: Option<Uuid>,
    status: Option<String>,
    is_cross_lab_borrowable: Option<bool>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
}

#[tracing::instrument(name = "Update an inventory item", skip(pool, payload), fields(user_id=%user_id, inventory_item_id=%inventory_item_id))]
pub async fn update_inventory_item(
    user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let inventory_item_id = inventory_item_id.into_inner();
    let existing = fetch_inventory_item(pool.get_ref(), inventory_item_id).await?;
    ensure_can_write(&actor, existing.laboratory_id)?;
    validate_location(pool.get_ref(), existing.laboratory_id, payload.location_id).await?;
    let status = match payload.status.as_deref() {
        Some(status) => Some(validate_status(status)?),
        None => None,
    };

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let item = sqlx::query_as::<_, InventoryItemRow>(
        r#"
        UPDATE asset_inventory_items
        SET
            batch_number = COALESCE($2, batch_number),
            location_id = COALESCE($3, location_id),
            status = COALESCE($4, status),
            is_cross_lab_borrowable = COALESCE($5, is_cross_lab_borrowable),
            public_notes = COALESCE($6, public_notes),
            internal_notes = COALESCE($7, internal_notes),
            updated_at = now()
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
    .bind(payload.batch_number.as_deref())
    .bind(payload.location_id)
    .bind(status)
    .bind(payload.is_cross_lab_borrowable)
    .bind(payload.public_notes.as_deref())
    .bind(payload.internal_notes.as_deref())
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_inventory_transaction(
        &mut transaction,
        &actor,
        InventoryTransactionData {
            inventory_item_id: Some(item.inventory_item_id),
            laboratory_id: item.laboratory_id,
            action: AuditAction::Update,
            quantity_delta: 0.0,
            allocated_delta: 0.0,
            from_location_id: existing.location_id,
            to_location_id: item.location_id,
            related_resource_type: None,
            related_resource_id: None,
            details: json!({ "status": item.status }),
        },
    )
    .await?;
    record_audit(
        &mut transaction,
        &actor,
        Some(item.laboratory_id),
        AuditAction::Update,
        AuditResource::InventoryItem,
        Some(item.inventory_item_id),
        json!({ "status": item.status }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(InventoryItemResponse::from_row(item, &actor)))
}
