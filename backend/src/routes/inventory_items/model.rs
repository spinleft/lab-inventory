use crate::domain::{AssetTrackingMode, InventoryStatus, UpdateInventoryItem};
use crate::utils::error_chain_fmt;
use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(super) struct DeletedAttachmentRow {
    pub(super) attachment_id: Uuid,
    pub(super) storage_key: String,
}

#[derive(Clone, sqlx::FromRow)]
pub(super) struct AssetForInventoryRow {
    pub(super) asset_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) tracking_mode: String,
    pub(super) default_unit_id: Uuid,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(crate) struct InventoryItemRow {
    pub(crate) inventory_item_id: Uuid,
    pub(crate) asset_id: Uuid,
    pub(crate) laboratory_id: Uuid,
    pub(crate) tracking_mode: String,
    pub(crate) serial_number: Option<String>,
    pub(crate) batch_number: Option<String>,
    pub(crate) quantity_on_hand: f64,
    pub(crate) quantity_allocated: f64,
    pub(crate) quantity_unit_id: Uuid,
    pub(crate) location_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) public_notes: Option<String>,
    pub(crate) internal_notes: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) last_stocktake_at: Option<DateTime<Utc>>,
    pub(crate) asset_category_id: Option<Uuid>,
    pub(crate) asset_name: String,
    pub(crate) asset_model: Option<String>,
    pub(crate) asset_manufacturer: Option<String>,
    pub(crate) asset_default_unit_id: Uuid,
}

#[derive(Clone, sqlx::FromRow)]
struct UnitRow {
    dimension: String,
    scale_to_base: f64,
}

#[derive(Serialize)]
pub(super) struct InventoryItemAssetResponse {
    asset_id: Uuid,
    category_id: Option<Uuid>,
    name: String,
    model: Option<String>,
    manufacturer: Option<String>,
    default_unit_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct InventoryItemResponse {
    inventory_item_id: Uuid,
    asset_id: Uuid,
    laboratory_id: Uuid,
    tracking_mode: String,
    serial_number: Option<String>,
    batch_number: Option<String>,
    quantity_on_hand: f64,
    quantity_allocated: f64,
    quantity_unit_id: Uuid,
    location_id: Option<Uuid>,
    status: String,
    public_notes: Option<String>,
    internal_notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_stocktake_at: Option<DateTime<Utc>>,
    asset: InventoryItemAssetResponse,
}

#[derive(thiserror::Error)]
pub(super) enum InventoryItemDatabaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for InventoryItemDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl InventoryItemResponse {
    pub(super) fn from_row(row: InventoryItemRow, include_internal_notes: bool) -> Self {
        Self {
            inventory_item_id: row.inventory_item_id,
            asset_id: row.asset_id,
            laboratory_id: row.laboratory_id,
            tracking_mode: row.tracking_mode,
            serial_number: row.serial_number,
            batch_number: row.batch_number,
            quantity_on_hand: row.quantity_on_hand,
            quantity_allocated: row.quantity_allocated,
            quantity_unit_id: row.quantity_unit_id,
            location_id: row.location_id,
            status: row.status,
            public_notes: row.public_notes,
            internal_notes: if include_internal_notes {
                row.internal_notes
            } else {
                None
            },
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_stocktake_at: row.last_stocktake_at,
            asset: InventoryItemAssetResponse {
                asset_id: row.asset_id,
                category_id: row.asset_category_id,
                name: row.asset_name,
                model: row.asset_model,
                manufacturer: row.asset_manufacturer,
                default_unit_id: row.asset_default_unit_id,
            },
        }
    }
}

pub(super) fn create_inventory_items_rollback_details(items: &[InventoryItemRow]) -> Value {
    let item_ids: Vec<_> = items.iter().map(|item| item.inventory_item_id).collect();
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "inventory_item",
            "where": {
                "inventory_item_ids": item_ids,
            },
        },
    })
}

pub(crate) fn update_inventory_item_rollback_details(item: &InventoryItemRow) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "inventory_item",
            "where": {
                "inventory_item_id": item.inventory_item_id,
            },
            "values": item,
        },
    })
}

pub(super) fn delete_inventory_item_rollback_details(
    item: &InventoryItemRow,
    attachment_ids: &[Uuid],
) -> Value {
    json!({
        "rollback": {
            "operation": "create",
            "resource_type": "inventory_item",
            "values": {
                "inventory_item": item,
                "deleted_attachment_ids": attachment_ids,
            },
        },
    })
}

pub(super) fn split_inventory_item_rollback_details(
    source_before: &InventoryItemRow,
    target_before: Option<&InventoryItemRow>,
    target_after: &InventoryItemRow,
) -> Value {
    json!({
        "rollback": {
            "operation": "split",
            "resource_type": "inventory_item",
            "source_before": source_before,
            "target_before": target_before,
            "target_after": target_after,
        },
    })
}

pub(super) fn merge_inventory_items_rollback_details(
    target_before: &InventoryItemRow,
    sources: &[InventoryItemRow],
    moved_attachment_ids: &[Uuid],
) -> Value {
    json!({
        "rollback": {
            "operation": "merge",
            "resource_type": "inventory_item",
            "target_before": target_before,
            "source_items": sources,
            "moved_attachment_ids": moved_attachment_ids,
        },
    })
}

pub(super) fn inventory_item_select() -> &'static str {
    r#"
    SELECT
        asset_inventory_items.inventory_item_id,
        asset_inventory_items.asset_id,
        asset_inventory_items.laboratory_id,
        asset_inventory_items.tracking_mode,
        asset_inventory_items.serial_number,
        asset_inventory_items.batch_number,
        asset_inventory_items.quantity_on_hand::double precision AS quantity_on_hand,
        asset_inventory_items.quantity_allocated::double precision AS quantity_allocated,
        asset_inventory_items.quantity_unit_id,
        asset_inventory_items.location_id,
        asset_inventory_items.status,
        asset_inventory_items.public_notes,
        asset_inventory_items.internal_notes,
        asset_inventory_items.created_at,
        asset_inventory_items.updated_at,
        asset_inventory_items.last_stocktake_at,
        assets.category_id AS asset_category_id,
        assets.name AS asset_name,
        assets.model AS asset_model,
        assets.manufacturer AS asset_manufacturer,
        assets.default_unit_id AS asset_default_unit_id
    FROM asset_inventory_items
    JOIN assets
      ON assets.asset_id = asset_inventory_items.asset_id
    "#
}

pub(super) async fn fetch_inventory_item(
    pool: &PgPool,
    inventory_item_id: Uuid,
) -> Result<Option<InventoryItemRow>, anyhow::Error> {
    sqlx::query_as!(
        InventoryItemRow,
        r#"
        SELECT
            asset_inventory_items.inventory_item_id,
            asset_inventory_items.asset_id,
            asset_inventory_items.laboratory_id,
            asset_inventory_items.tracking_mode,
            asset_inventory_items.serial_number,
            asset_inventory_items.batch_number,
            asset_inventory_items.quantity_on_hand::double precision AS "quantity_on_hand!",
            asset_inventory_items.quantity_allocated::double precision AS "quantity_allocated!",
            asset_inventory_items.quantity_unit_id,
            asset_inventory_items.location_id,
            asset_inventory_items.status,
            asset_inventory_items.public_notes,
            asset_inventory_items.internal_notes,
            asset_inventory_items.created_at,
            asset_inventory_items.updated_at,
            asset_inventory_items.last_stocktake_at,
            assets.category_id AS asset_category_id,
            assets.name AS asset_name,
            assets.model AS asset_model,
            assets.manufacturer AS asset_manufacturer,
            assets.default_unit_id AS asset_default_unit_id
        FROM asset_inventory_items
        JOIN assets
          ON assets.asset_id = asset_inventory_items.asset_id
        WHERE asset_inventory_items.inventory_item_id = $1
        "#,
        inventory_item_id,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch inventory item")
}

pub(crate) async fn fetch_inventory_item_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
) -> Result<Option<InventoryItemRow>, anyhow::Error> {
    sqlx::query_as!(
        InventoryItemRow,
        r#"
        SELECT
            asset_inventory_items.inventory_item_id,
            asset_inventory_items.asset_id,
            asset_inventory_items.laboratory_id,
            asset_inventory_items.tracking_mode,
            asset_inventory_items.serial_number,
            asset_inventory_items.batch_number,
            asset_inventory_items.quantity_on_hand::double precision AS "quantity_on_hand!",
            asset_inventory_items.quantity_allocated::double precision AS "quantity_allocated!",
            asset_inventory_items.quantity_unit_id,
            asset_inventory_items.location_id,
            asset_inventory_items.status,
            asset_inventory_items.public_notes,
            asset_inventory_items.internal_notes,
            asset_inventory_items.created_at,
            asset_inventory_items.updated_at,
            asset_inventory_items.last_stocktake_at,
            assets.category_id AS asset_category_id,
            assets.name AS asset_name,
            assets.model AS asset_model,
            assets.manufacturer AS asset_manufacturer,
            assets.default_unit_id AS asset_default_unit_id
        FROM asset_inventory_items
        JOIN assets
          ON assets.asset_id = asset_inventory_items.asset_id
        WHERE asset_inventory_items.inventory_item_id = $1
        FOR UPDATE OF asset_inventory_items
        "#,
        inventory_item_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch inventory item for update")
}

pub(super) async fn fetch_inventory_items_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_ids: &[Uuid],
) -> Result<Vec<InventoryItemRow>, anyhow::Error> {
    sqlx::query_as!(
        InventoryItemRow,
        r#"
        SELECT
            asset_inventory_items.inventory_item_id,
            asset_inventory_items.asset_id,
            asset_inventory_items.laboratory_id,
            asset_inventory_items.tracking_mode,
            asset_inventory_items.serial_number,
            asset_inventory_items.batch_number,
            asset_inventory_items.quantity_on_hand::double precision AS "quantity_on_hand!",
            asset_inventory_items.quantity_allocated::double precision AS "quantity_allocated!",
            asset_inventory_items.quantity_unit_id,
            asset_inventory_items.location_id,
            asset_inventory_items.status,
            asset_inventory_items.public_notes,
            asset_inventory_items.internal_notes,
            asset_inventory_items.created_at,
            asset_inventory_items.updated_at,
            asset_inventory_items.last_stocktake_at,
            assets.category_id AS asset_category_id,
            assets.name AS asset_name,
            assets.model AS asset_model,
            assets.manufacturer AS asset_manufacturer,
            assets.default_unit_id AS asset_default_unit_id
        FROM asset_inventory_items
        JOIN assets
          ON assets.asset_id = asset_inventory_items.asset_id
        WHERE asset_inventory_items.inventory_item_id = ANY($1)
        ORDER BY asset_inventory_items.inventory_item_id
        FOR UPDATE OF asset_inventory_items
        "#,
        inventory_item_ids,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch inventory items for update")
}

pub(super) async fn fetch_asset_laboratory_id(
    pool: &PgPool,
    asset_id: Uuid,
) -> Result<Option<Uuid>, anyhow::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT laboratory_id
        FROM assets
        WHERE asset_id = $1
        "#,
        asset_id,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch asset")
}

pub(super) async fn fetch_asset_for_inventory_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
) -> Result<Option<AssetForInventoryRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetForInventoryRow,
        r#"
        SELECT
            asset_id,
            laboratory_id,
            tracking_mode,
            default_unit_id
        FROM assets
        WHERE asset_id = $1
        FOR UPDATE
        "#,
        asset_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch asset for update")
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn find_quantity_aggregate_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    asset_id: Uuid,
    batch_number: Option<&str>,
    location_id: Option<Uuid>,
    status: &str,
    quantity_unit_id: Uuid,
    exclude_inventory_item_id: Option<Uuid>,
) -> Result<Option<InventoryItemRow>, anyhow::Error> {
    sqlx::query_as!(
        InventoryItemRow,
        r#"
        SELECT
            asset_inventory_items.inventory_item_id,
            asset_inventory_items.asset_id,
            asset_inventory_items.laboratory_id,
            asset_inventory_items.tracking_mode,
            asset_inventory_items.serial_number,
            asset_inventory_items.batch_number,
            asset_inventory_items.quantity_on_hand::double precision AS "quantity_on_hand!",
            asset_inventory_items.quantity_allocated::double precision AS "quantity_allocated!",
            asset_inventory_items.quantity_unit_id,
            asset_inventory_items.location_id,
            asset_inventory_items.status,
            asset_inventory_items.public_notes,
            asset_inventory_items.internal_notes,
            asset_inventory_items.created_at,
            asset_inventory_items.updated_at,
            asset_inventory_items.last_stocktake_at,
            assets.category_id AS asset_category_id,
            assets.name AS asset_name,
            assets.model AS asset_model,
            assets.manufacturer AS asset_manufacturer,
            assets.default_unit_id AS asset_default_unit_id
        FROM asset_inventory_items
        JOIN assets
          ON assets.asset_id = asset_inventory_items.asset_id
        WHERE asset_inventory_items.tracking_mode = 'quantity'
          AND asset_inventory_items.laboratory_id = $1
          AND asset_inventory_items.asset_id = $2
          AND asset_inventory_items.batch_number IS NOT DISTINCT FROM $3
          AND asset_inventory_items.location_id IS NOT DISTINCT FROM $4
          AND asset_inventory_items.status = $5
          AND asset_inventory_items.quantity_unit_id = $6
          AND ($7::uuid IS NULL OR asset_inventory_items.inventory_item_id <> $7)
        FOR UPDATE OF asset_inventory_items
        "#,
        laboratory_id,
        asset_id,
        batch_number,
        location_id,
        status,
        quantity_unit_id,
        exclude_inventory_item_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch quantity aggregate for update")
}

pub(super) async fn next_serial_numbers(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    count: u16,
) -> Result<Vec<String>, anyhow::Error> {
    let max_serial = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(MAX(substring(serial_number from 2)::integer), 0) AS "max_serial!"
        FROM asset_inventory_items
        WHERE asset_id = $1
          AND serial_number ~ '^#[0-9]+$'
        "#,
        asset_id,
    )
    .fetch_one(transaction.as_mut())
    .await
    .context("Failed to fetch the next inventory item serial number")?;

    Ok((1..=count)
        .map(|offset| format!("#{}", i64::from(max_serial) + i64::from(offset)))
        .collect())
}

pub(super) fn validate_requested_ids(
    requested_ids: &[Uuid],
    rows: &[InventoryItemRow],
) -> Result<(), InventoryItemDatabaseError> {
    if requested_ids.is_empty() {
        return Err(InventoryItemDatabaseError::Validation(
            "inventory_item_ids cannot be empty".into(),
        ));
    }
    let unique_ids: HashSet<_> = requested_ids.iter().copied().collect();
    if unique_ids.len() != requested_ids.len() {
        return Err(InventoryItemDatabaseError::Validation(
            "inventory_item_ids cannot contain duplicates".into(),
        ));
    }
    if rows.len() != requested_ids.len() {
        return Err(InventoryItemDatabaseError::NotFound(
            "One or more inventory items were not found".into(),
        ));
    }

    Ok(())
}

pub(super) fn validate_status(status: Option<String>) -> Result<Option<String>, String> {
    status
        .map(|status| InventoryStatus::parse(&status).map(|status| status.as_str().to_string()))
        .transpose()
}

pub(super) fn validate_quantity_item(row: &InventoryItemRow) -> Result<(), String> {
    if row.tracking_mode == "quantity" {
        Ok(())
    } else {
        Err("Operation only applies to quantity-tracked inventory items".into())
    }
}

pub(super) fn validate_quantities(
    quantity_on_hand: f64,
    quantity_allocated: f64,
) -> Result<(), String> {
    if quantity_on_hand < 0.0 {
        return Err("quantity_on_hand must be non-negative".into());
    }
    if quantity_allocated < 0.0 {
        return Err("quantity_allocated must be non-negative".into());
    }
    if quantity_allocated > quantity_on_hand {
        return Err("quantity_allocated cannot exceed quantity_on_hand".into());
    }

    Ok(())
}

pub(super) fn resolve_asset_quantity_unit(
    requested_unit_id: Option<Uuid>,
    asset_default_unit_id: Uuid,
) -> Result<Uuid, String> {
    if requested_unit_id.is_some_and(|unit_id| unit_id != asset_default_unit_id) {
        return Err("Inventory item unit must match asset default unit".into());
    }

    Ok(asset_default_unit_id)
}

pub(super) async fn validate_location(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    location_id: Uuid,
) -> Result<(), InventoryItemDatabaseError> {
    let found = sqlx::query_scalar!(
        r#"
        SELECT location_id
        FROM locations
        WHERE laboratory_id = $1
          AND location_id = $2
        FOR UPDATE
        "#,
        laboratory_id,
        location_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch location")?;

    if found.is_some() {
        Ok(())
    } else {
        Err(InventoryItemDatabaseError::Validation(
            "Inventory item location does not belong to this laboratory".into(),
        ))
    }
}

pub(super) async fn convert_quantity_between_units(
    transaction: &mut Transaction<'_, Postgres>,
    source_unit_id: Uuid,
    target_unit_id: Uuid,
    source_quantity: f64,
) -> Result<f64, InventoryItemDatabaseError> {
    if source_unit_id == target_unit_id {
        return Ok(source_quantity);
    }
    let source_unit = fetch_unit_for_update(transaction, source_unit_id).await?;
    let target_unit = fetch_unit_for_update(transaction, target_unit_id).await?;
    if source_unit.dimension != target_unit.dimension {
        return Err(InventoryItemDatabaseError::Validation(
            "Target unit dimension does not match source unit dimension".into(),
        ));
    }

    Ok(source_quantity * source_unit.scale_to_base / target_unit.scale_to_base)
}

async fn fetch_unit_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> Result<UnitRow, InventoryItemDatabaseError> {
    sqlx::query_as!(
        UnitRow,
        r#"
        SELECT dimension, scale_to_base
        FROM units
        WHERE unit_id = $1
        FOR UPDATE
        "#,
        unit_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch unit")?
    .ok_or(InventoryItemDatabaseError::Validation(
        "Unit not found".into(),
    ))
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Saving new inventory item in the database",
    skip(
        transaction,
        tracking_mode,
        serial_number,
        batch_number,
        status,
        public_notes,
        internal_notes
    ),
    fields(asset_id=%asset_id)
)]
pub(super) async fn insert_inventory_item(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    laboratory_id: Uuid,
    tracking_mode: &str,
    serial_number: Option<&str>,
    batch_number: Option<&str>,
    quantity_on_hand: f64,
    quantity_allocated: f64,
    quantity_unit_id: Uuid,
    location_id: Option<Uuid>,
    status: &str,
    public_notes: Option<&str>,
    internal_notes: Option<&str>,
) -> Result<InventoryItemRow, InventoryItemDatabaseError> {
    let inventory_item_id = Uuid::new_v4();
    sqlx::query!(
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
            quantity_unit_id,
            location_id,
            status,
            public_notes,
            internal_notes
        )
        VALUES (
            $1, $2, $3, $4, $5, $6,
            $7::double precision::numeric,
            $8::double precision::numeric,
            $9, $10, $11, $12, $13
        )
        "#,
        inventory_item_id,
        asset_id,
        laboratory_id,
        tracking_mode,
        trim_optional(serial_number),
        trim_optional(batch_number),
        quantity_on_hand,
        quantity_allocated,
        quantity_unit_id,
        location_id,
        status,
        trim_optional(public_notes),
        trim_optional(internal_notes),
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    fetch_inventory_item_for_update(transaction, inventory_item_id)
        .await?
        .ok_or(InventoryItemDatabaseError::Unexpected(anyhow!(
            "Created inventory item not found"
        )))
}

pub(super) async fn apply_inventory_item_patch(
    transaction: &mut Transaction<'_, Postgres>,
    existing: &InventoryItemRow,
    patch: UpdateInventoryItem,
) -> Result<InventoryItemRow, InventoryItemDatabaseError> {
    let tracking_mode = AssetTrackingMode::parse(&existing.tracking_mode)
        .map_err(InventoryItemDatabaseError::Validation)?;
    validate_patch_against_tracking_mode(&patch, tracking_mode)?;

    let serial_number = match patch.serial_number {
        Some(serial_number) => Some(String::from(serial_number)),
        None => existing.serial_number.clone(),
    };
    let batch_number = patch.batch_number.resolve(existing.batch_number.clone());
    let location_id = patch
        .location_id
        .resolve(existing.location_id.map(Uuid::into))
        .map(Uuid::from);
    if let Some(location_id) = location_id {
        validate_location(transaction, existing.laboratory_id, location_id).await?;
    }
    let status = patch
        .status
        .map(|status| status.as_str().to_string())
        .unwrap_or_else(|| existing.status.clone());
    let quantity_unit_id = match tracking_mode {
        AssetTrackingMode::Serialized => existing.quantity_unit_id,
        AssetTrackingMode::Quantity => resolve_asset_quantity_unit(
            patch.quantity_unit_id.map(Uuid::from),
            existing.asset_default_unit_id,
        )
        .map_err(InventoryItemDatabaseError::Validation)?,
    };
    let mut quantity_on_hand = patch.quantity_on_hand.unwrap_or(existing.quantity_on_hand);
    let mut quantity_allocated = patch
        .quantity_allocated
        .unwrap_or(existing.quantity_allocated);
    if tracking_mode == AssetTrackingMode::Quantity && existing.quantity_unit_id != quantity_unit_id
    {
        quantity_on_hand = convert_quantity_between_units(
            transaction,
            existing.quantity_unit_id,
            quantity_unit_id,
            quantity_on_hand,
        )
        .await?;
        quantity_allocated = convert_quantity_between_units(
            transaction,
            existing.quantity_unit_id,
            quantity_unit_id,
            quantity_allocated,
        )
        .await?;
    }
    let public_notes = patch.public_notes.resolve(existing.public_notes.clone());
    let internal_notes = patch
        .internal_notes
        .resolve(existing.internal_notes.clone());
    validate_quantities(quantity_on_hand, quantity_allocated)
        .map_err(InventoryItemDatabaseError::Validation)?;

    sqlx::query!(
        r#"
        UPDATE asset_inventory_items
        SET
            serial_number = $2,
            batch_number = $3,
            quantity_on_hand = $4::double precision::numeric,
            quantity_allocated = $5::double precision::numeric,
            quantity_unit_id = $6,
            location_id = $7,
            status = $8,
            public_notes = $9,
            internal_notes = $10,
            updated_at = now()
        WHERE inventory_item_id = $1
        "#,
        existing.inventory_item_id,
        serial_number.as_deref(),
        batch_number.as_deref(),
        quantity_on_hand,
        quantity_allocated,
        quantity_unit_id,
        location_id,
        status,
        public_notes.as_deref(),
        internal_notes.as_deref(),
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    fetch_inventory_item_for_update(transaction, existing.inventory_item_id)
        .await?
        .ok_or(InventoryItemDatabaseError::Unexpected(anyhow!(
            "Updated inventory item not found"
        )))
}

fn validate_patch_against_tracking_mode(
    patch: &UpdateInventoryItem,
    tracking_mode: AssetTrackingMode,
) -> Result<(), InventoryItemDatabaseError> {
    match tracking_mode {
        AssetTrackingMode::Serialized => {
            if patch.quantity_on_hand.is_some()
                || patch.quantity_allocated.is_some()
                || patch.quantity_unit_id.is_some()
            {
                return Err(InventoryItemDatabaseError::Validation(
                    "Serialized inventory items cannot update quantity fields".into(),
                ));
            }
        }
        AssetTrackingMode::Quantity => {
            if patch.serial_number.is_some() {
                return Err(InventoryItemDatabaseError::Validation(
                    "Quantity-tracked inventory items cannot update serial_number".into(),
                ));
            }
        }
    }

    Ok(())
}

#[tracing::instrument(
    name = "Updating inventory item quantity in the database",
    skip(transaction),
    fields(inventory_item_id=%inventory_item_id)
)]
pub(super) async fn set_quantity_on_hand(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    quantity_on_hand: f64,
) -> Result<InventoryItemRow, InventoryItemDatabaseError> {
    sqlx::query!(
        r#"
        UPDATE asset_inventory_items
        SET
            quantity_on_hand = $2::double precision::numeric,
            updated_at = now()
        WHERE inventory_item_id = $1
        "#,
        inventory_item_id,
        quantity_on_hand,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    fetch_inventory_item_for_update(transaction, inventory_item_id)
        .await?
        .ok_or(InventoryItemDatabaseError::Unexpected(anyhow!(
            "Updated inventory item not found"
        )))
}

#[tracing::instrument(
    name = "Adding inventory item quantities in the database",
    skip(transaction),
    fields(inventory_item_id=%inventory_item_id)
)]
pub(super) async fn add_quantities_to_item(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    quantity_delta: f64,
    allocated_delta: f64,
) -> Result<InventoryItemRow, InventoryItemDatabaseError> {
    sqlx::query!(
        r#"
        UPDATE asset_inventory_items
        SET
            quantity_on_hand = quantity_on_hand + $2::double precision::numeric,
            quantity_allocated = quantity_allocated + $3::double precision::numeric,
            updated_at = now()
        WHERE inventory_item_id = $1
        "#,
        inventory_item_id,
        quantity_delta,
        allocated_delta,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    fetch_inventory_item_for_update(transaction, inventory_item_id)
        .await?
        .ok_or(InventoryItemDatabaseError::Unexpected(anyhow!(
            "Updated inventory item not found"
        )))
}

#[tracing::instrument(
    name = "Deleting inventory item attachments from the database",
    skip(transaction),
    fields(inventory_item_id=%inventory_item_id)
)]
pub(super) async fn delete_inventory_item_attachments(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
) -> Result<Vec<DeletedAttachmentRow>, anyhow::Error> {
    sqlx::query_as!(
        DeletedAttachmentRow,
        r#"
        WITH deleted_assignments AS (
            DELETE FROM asset_attachment_assignments
            WHERE inventory_item_id = $1
            RETURNING attachment_id, file_id
        ),
        deleted_files AS (
            DELETE FROM files
            WHERE file_id IN (SELECT file_id FROM deleted_assignments)
            RETURNING file_id, storage_key
        )
        SELECT deleted_assignments.attachment_id, deleted_files.storage_key
        FROM deleted_assignments
        JOIN deleted_files ON deleted_files.file_id = deleted_assignments.file_id
        ORDER BY deleted_assignments.attachment_id
        "#,
        inventory_item_id,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to delete inventory item attachments")
}

#[tracing::instrument(
    name = "Moving inventory item attachments in the database",
    skip(transaction, from_inventory_item_ids),
    fields(to_inventory_item_id=%to_inventory_item_id)
)]
pub(super) async fn move_inventory_item_attachments(
    transaction: &mut Transaction<'_, Postgres>,
    from_inventory_item_ids: &[Uuid],
    to_inventory_item_id: Uuid,
) -> Result<Vec<Uuid>, anyhow::Error> {
    sqlx::query_scalar!(
        r#"
        UPDATE asset_attachment_assignments
        SET inventory_item_id = $2,
            updated_at = now()
        WHERE inventory_item_id = ANY($1)
        RETURNING attachment_id
        "#,
        from_inventory_item_ids,
        to_inventory_item_id,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to move inventory item attachments")
}

#[tracing::instrument(
    name = "Deleting inventory item from the database",
    skip(transaction),
    fields(inventory_item_id=%inventory_item_id)
)]
pub(super) async fn delete_inventory_item_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
) -> Result<(), InventoryItemDatabaseError> {
    sqlx::query!(
        r#"
        DELETE FROM asset_inventory_items
        WHERE inventory_item_id = $1
        "#,
        inventory_item_id,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

pub(super) fn map_database_error(error: sqlx::Error) -> InventoryItemDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("idx_asset_inventory_items_unique_asset_serial_number")) => {
                return InventoryItemDatabaseError::Conflict(
                    "Inventory item serial number already exists for this asset".into(),
                );
            }
            (Some("23505"), Some("idx_asset_inventory_items_unique_quantity_aggregate")) => {
                return InventoryItemDatabaseError::Conflict(
                    "Inventory item already exists for this quantity aggregate".into(),
                );
            }
            (Some("23505"), _) => {
                return InventoryItemDatabaseError::Conflict(
                    "Inventory item already exists".into(),
                );
            }
            (Some("23503"), _) => {
                return InventoryItemDatabaseError::Validation("Invalid referenced record".into());
            }
            (Some("23514"), _) => {
                return InventoryItemDatabaseError::Validation(
                    "Invalid inventory item data".into(),
                );
            }
            _ => {}
        }
    }

    InventoryItemDatabaseError::Unexpected(error.into())
}

fn trim_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
