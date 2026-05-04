use super::model::{AssetForInventory, InventoryItemRow};
use crate::audit::AuditAction;
use crate::authentication::Actor;
use crate::utils::ApiError;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(super) async fn fetch_inventory_item(
    pool: &PgPool,
    inventory_item_id: Uuid,
) -> Result<InventoryItemRow, ApiError> {
    sqlx::query_as::<_, InventoryItemRow>(INVENTORY_ITEM_SELECT)
        .bind(inventory_item_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound)
}

const INVENTORY_ITEM_SELECT: &str = r#"
SELECT
    asset_inventory_items.inventory_item_id,
    asset_inventory_items.asset_id,
    assets.name AS asset_name,
    assets.model AS asset_model,
    asset_inventory_items.laboratory_id,
    laboratories.name AS laboratory_name,
    asset_inventory_items.tracking_mode,
    asset_inventory_items.serial_number,
    asset_inventory_items.batch_number,
    asset_inventory_items.quantity_on_hand,
    asset_inventory_items.quantity_allocated,
    asset_inventory_items.unit_id,
    units.code AS unit_code,
    units.allow_decimal AS unit_allow_decimal,
    asset_inventory_items.location_id,
    locations.name AS location_name,
    asset_inventory_items.status,
    asset_inventory_items.is_cross_lab_borrowable,
    asset_inventory_items.public_notes,
    asset_inventory_items.internal_notes,
    asset_inventory_items.created_at,
    asset_inventory_items.updated_at
FROM asset_inventory_items
INNER JOIN assets USING (asset_id)
INNER JOIN laboratories ON laboratories.laboratory_id = asset_inventory_items.laboratory_id
INNER JOIN units ON units.unit_id = asset_inventory_items.unit_id
LEFT JOIN locations ON locations.location_id = asset_inventory_items.location_id
WHERE asset_inventory_items.inventory_item_id = $1
"#;

pub(super) fn inventory_list_select() -> &'static str {
    r#"
    SELECT
        asset_inventory_items.inventory_item_id,
        asset_inventory_items.asset_id,
        assets.name AS asset_name,
        assets.model AS asset_model,
        asset_inventory_items.laboratory_id,
        laboratories.name AS laboratory_name,
        asset_inventory_items.tracking_mode,
        asset_inventory_items.serial_number,
        asset_inventory_items.batch_number,
        asset_inventory_items.quantity_on_hand,
        asset_inventory_items.quantity_allocated,
        asset_inventory_items.unit_id,
        units.code AS unit_code,
        units.allow_decimal AS unit_allow_decimal,
        asset_inventory_items.location_id,
        locations.name AS location_name,
        asset_inventory_items.status,
        asset_inventory_items.is_cross_lab_borrowable,
        asset_inventory_items.public_notes,
        asset_inventory_items.internal_notes,
        asset_inventory_items.created_at,
        asset_inventory_items.updated_at
    FROM asset_inventory_items
    INNER JOIN assets USING (asset_id)
    INNER JOIN laboratories ON laboratories.laboratory_id = asset_inventory_items.laboratory_id
    INNER JOIN units ON units.unit_id = asset_inventory_items.unit_id
    LEFT JOIN locations ON locations.location_id = asset_inventory_items.location_id
    "#
}

pub(super) async fn fetch_asset_for_inventory(
    pool: &PgPool,
    asset_id: Uuid,
) -> Result<AssetForInventory, ApiError> {
    sqlx::query_as::<_, AssetForInventory>(
        r#"
        SELECT asset_id, laboratory_id, tracking_mode, default_unit_id
        FROM assets
        WHERE asset_id = $1
        "#,
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or_else(|| ApiError::BadRequest("Unknown asset".into()))
}

pub(super) fn ensure_can_write(actor: &Actor, laboratory_id: Uuid) -> Result<(), ApiError> {
    if actor.can_write_laboratory_resource(laboratory_id) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) async fn validate_location(
    pool: &PgPool,
    laboratory_id: Uuid,
    location_id: Option<Uuid>,
) -> Result<(), ApiError> {
    if let Some(location_id) = location_id {
        let location_laboratory_id: Option<Uuid> =
            sqlx::query_scalar("SELECT laboratory_id FROM locations WHERE location_id = $1")
                .bind(location_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ApiError::UnexpectedError(e.into()))?;
        match location_laboratory_id {
            Some(location_laboratory_id) if location_laboratory_id == laboratory_id => Ok(()),
            Some(_) => Err(ApiError::BadRequest(
                "location_id belongs to another laboratory".into(),
            )),
            None => Err(ApiError::BadRequest("Unknown location".into())),
        }
    } else {
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
pub(super) struct UnitForValidation {
    pub unit_id: Uuid,
    pub code: String,
    pub dimension: String,
    pub allow_decimal: bool,
}

pub(super) async fn fetch_unit(
    pool: &PgPool,
    unit_id: Uuid,
) -> Result<UnitForValidation, ApiError> {
    sqlx::query_as::<_, UnitForValidation>(
        "SELECT unit_id, code, dimension, allow_decimal FROM units WHERE unit_id = $1",
    )
    .bind(unit_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or_else(|| ApiError::BadRequest("Unknown unit".into()))
}

pub(super) async fn validate_unit_for_asset(
    pool: &PgPool,
    asset: &AssetForInventory,
    unit_id: Uuid,
) -> Result<UnitForValidation, ApiError> {
    let default_unit = fetch_unit(pool, asset.default_unit_id).await?;
    let unit = fetch_unit(pool, unit_id).await?;
    if asset.tracking_mode == "serialized" && unit.code != "pcs" {
        return Err(ApiError::BadRequest(
            "serialized inventory items must use pcs".into(),
        ));
    }
    if default_unit.dimension != unit.dimension {
        return Err(ApiError::BadRequest(
            "unit dimension does not match the asset default unit".into(),
        ));
    }
    Ok(unit)
}

pub(super) fn validate_status(status: &str) -> Result<&'static str, ApiError> {
    match status.trim() {
        "available" => Ok("available"),
        "reserved" => Ok("reserved"),
        "borrowed" => Ok("borrowed"),
        "maintenance" => Ok("maintenance"),
        "retired" => Ok("retired"),
        "lost" => Ok("lost"),
        "consumed" => Ok("consumed"),
        _ => Err(ApiError::BadRequest("Unknown inventory status".into())),
    }
}

pub(super) fn validate_quantity(
    quantity: f64,
    field: &str,
    allow_decimal: bool,
) -> Result<(), ApiError> {
    if !quantity.is_finite() || quantity < 0.0 {
        return Err(ApiError::BadRequest(format!(
            "{field} must be non-negative"
        )));
    }
    if !allow_decimal && quantity.fract().abs() > f64::EPSILON {
        return Err(ApiError::BadRequest(format!("{field} must be an integer")));
    }
    Ok(())
}

pub(super) fn validate_positive_quantity(
    quantity: f64,
    field: &str,
    allow_decimal: bool,
) -> Result<(), ApiError> {
    validate_quantity(quantity, field, allow_decimal)?;
    if quantity <= 0.0 {
        return Err(ApiError::BadRequest(format!("{field} must be positive")));
    }
    Ok(())
}

pub(super) fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => return ApiError::Conflict("Inventory item already exists".into()),
            Some("23503") => {
                return ApiError::Conflict("Inventory item is still referenced".into());
            }
            Some("23514") => return ApiError::BadRequest("Invalid inventory item data".into()),
            _ => {}
        }
    }
    ApiError::UnexpectedError(error.into())
}

pub(super) struct InventoryTransactionData {
    pub inventory_item_id: Option<Uuid>,
    pub laboratory_id: Uuid,
    pub action: AuditAction,
    pub quantity_delta: f64,
    pub allocated_delta: f64,
    pub from_location_id: Option<Uuid>,
    pub to_location_id: Option<Uuid>,
    pub related_resource_type: Option<&'static str>,
    pub related_resource_id: Option<Uuid>,
    pub details: Value,
}

pub(super) async fn record_inventory_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    data: InventoryTransactionData,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO inventory_transactions (
            transaction_id,
            inventory_item_id,
            laboratory_id,
            actor_user_id,
            actor_laboratory_id,
            action,
            quantity_delta,
            allocated_delta,
            from_location_id,
            to_location_id,
            related_resource_type,
            related_resource_id,
            details
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(data.inventory_item_id)
    .bind(data.laboratory_id)
    .bind(actor.user_id)
    .bind(actor.laboratory_id)
    .bind(data.action.as_str())
    .bind(data.quantity_delta)
    .bind(data.allocated_delta)
    .bind(data.from_location_id)
    .bind(data.to_location_id)
    .bind(data.related_resource_type)
    .bind(data.related_resource_id)
    .bind(data.details)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    Ok(())
}
