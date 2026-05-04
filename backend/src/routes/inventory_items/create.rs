use super::helpers::{
    InventoryTransactionData, ensure_can_write, fetch_asset_for_inventory, map_database_error,
    record_inventory_transaction, validate_location, validate_positive_quantity, validate_quantity,
    validate_status, validate_unit_for_asset,
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
    asset_id: Uuid,
    serial_number: Option<String>,
    batch_number: Option<String>,
    quantity_on_hand: Option<f64>,
    quantity_allocated: Option<f64>,
    unit_id: Option<Uuid>,
    location_id: Option<Uuid>,
    status: Option<String>,
    is_cross_lab_borrowable: Option<bool>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
}

#[tracing::instrument(name = "Create an inventory item", skip(request, pool, payload), fields(user_id=%user_id))]
pub async fn create_inventory_item(
    request: HttpRequest,
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let idempotency_key = idempotency_key_from_request(&request)?;
    let asset = fetch_asset_for_inventory(pool.get_ref(), payload.asset_id).await?;
    ensure_can_write(&actor, asset.laboratory_id)?;
    validate_location(pool.get_ref(), asset.laboratory_id, payload.location_id).await?;
    let unit_id = payload.unit_id.unwrap_or(asset.default_unit_id);
    let unit = validate_unit_for_asset(pool.get_ref(), &asset, unit_id).await?;
    let status = match payload.status.as_deref() {
        Some(status) => validate_status(status)?,
        None => "available",
    };

    let serial_number = payload
        .serial_number
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let (quantity_on_hand, quantity_allocated) = if asset.tracking_mode == "serialized" {
        let serial_number = serial_number
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("serial_number is required".into()))?;
        if payload
            .quantity_on_hand
            .is_some_and(|quantity| quantity != 1.0)
        {
            return Err(ApiError::BadRequest(
                "serialized inventory quantity_on_hand must be 1".into(),
            ));
        }
        let quantity_allocated = payload.quantity_allocated.unwrap_or(0.0);
        if quantity_allocated != 0.0 && quantity_allocated != 1.0 {
            return Err(ApiError::BadRequest(
                "serialized inventory quantity_allocated must be 0 or 1".into(),
            ));
        }
        if serial_number.is_empty() {
            return Err(ApiError::BadRequest("serial_number is required".into()));
        }
        (1.0, quantity_allocated)
    } else {
        if serial_number.is_some() {
            return Err(ApiError::BadRequest(
                "quantity inventory items cannot have serial_number".into(),
            ));
        }
        let quantity_on_hand = payload
            .quantity_on_hand
            .ok_or_else(|| ApiError::BadRequest("quantity_on_hand is required".into()))?;
        let quantity_allocated = payload.quantity_allocated.unwrap_or(0.0);
        validate_positive_quantity(quantity_on_hand, "quantity_on_hand", unit.allow_decimal)?;
        validate_quantity(quantity_allocated, "quantity_allocated", unit.allow_decimal)?;
        if quantity_allocated > quantity_on_hand {
            return Err(ApiError::BadRequest(
                "quantity_allocated cannot exceed quantity_on_hand".into(),
            ));
        }
        (quantity_on_hand, quantity_allocated)
    };

    match try_processing(pool.get_ref(), &idempotency_key, *user_id).await? {
        NextAction::ReturnSavedResponse(response) => Ok(response),
        NextAction::StartProcessing(mut transaction) => {
            let item = sqlx::query_as::<_, InventoryItemRow>(
                r#"
                INSERT INTO asset_inventory_items (
                    inventory_item_id,
                    asset_id,
                    laboratory_id,
                    tracking_mode,
                    serial_number,
                    batch_number,
                    quantity_on_hand,
                    quantity_allocated,
                    unit_id,
                    location_id,
                    status,
                    is_cross_lab_borrowable,
                    public_notes,
                    internal_notes
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                RETURNING
                    inventory_item_id,
                    asset_id,
                    (SELECT name FROM assets WHERE asset_id = $2) AS asset_name,
                    (SELECT model FROM assets WHERE asset_id = $2) AS asset_model,
                    laboratory_id,
                    (SELECT name FROM laboratories WHERE laboratory_id = $3) AS laboratory_name,
                    tracking_mode,
                    serial_number,
                    batch_number,
                    quantity_on_hand,
                    quantity_allocated,
                    unit_id,
                    (SELECT code FROM units WHERE unit_id = $9) AS unit_code,
                    (SELECT allow_decimal FROM units WHERE unit_id = $9) AS unit_allow_decimal,
                    location_id,
                    (SELECT name FROM locations WHERE location_id = $10) AS location_name,
                    status,
                    is_cross_lab_borrowable,
                    public_notes,
                    internal_notes,
                    created_at,
                    updated_at
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(asset.asset_id)
            .bind(asset.laboratory_id)
            .bind(&asset.tracking_mode)
            .bind(serial_number.as_deref())
            .bind(payload.batch_number.as_deref())
            .bind(quantity_on_hand)
            .bind(quantity_allocated)
            .bind(unit.unit_id)
            .bind(payload.location_id)
            .bind(status)
            .bind(payload.is_cross_lab_borrowable.unwrap_or(false))
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
                    action: AuditAction::Create,
                    quantity_delta: item.quantity_on_hand,
                    allocated_delta: item.quantity_allocated,
                    from_location_id: None,
                    to_location_id: item.location_id,
                    related_resource_type: None,
                    related_resource_id: None,
                    details: json!({ "asset_id": item.asset_id, "serial_number": item.serial_number }),
                },
            )
            .await?;
            record_audit(
                &mut transaction,
                &actor,
                Some(item.laboratory_id),
                AuditAction::Create,
                AuditResource::InventoryItem,
                Some(item.inventory_item_id),
                json!({ "asset_id": item.asset_id, "serial_number": item.serial_number }),
            )
            .await?;

            let response =
                HttpResponse::Created().json(InventoryItemResponse::from_row(item, &actor));
            save_response(transaction, &idempotency_key, *user_id, response).await
        }
    }
}
