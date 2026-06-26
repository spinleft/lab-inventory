use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetInventoryStatus, AssetTrackingMode, LaboratoryId, NullableUpdate, UserId,
};
use crate::routes::PaginationError;
use crate::routes::attachments::DeletedAttachmentRow;
use crate::utils::error_chain_fmt;
use actix_web::ResponseError;
use actix_web::http::StatusCode;
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum InventoryItemError {
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

impl std::fmt::Debug for InventoryItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for InventoryItemError {
    fn status_code(&self) -> StatusCode {
        match self {
            InventoryItemError::ValidationError(_) => StatusCode::BAD_REQUEST,
            InventoryItemError::Forbidden(_) => StatusCode::FORBIDDEN,
            InventoryItemError::NotFound(_) => StatusCode::NOT_FOUND,
            InventoryItemError::ConflictError(_) => StatusCode::CONFLICT,
            InventoryItemError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<PaginationError> for InventoryItemError {
    fn from(error: PaginationError) -> Self {
        Self::ValidationError(error.to_string())
    }
}

#[derive(Clone, sqlx::FromRow)]
pub(super) struct AssetForInventoryRow {
    pub(super) asset_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) tracking_mode: String,
    pub(super) default_unit_id: Uuid,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct InventoryItemRow {
    pub(super) inventory_item_id: Uuid,
    pub(super) asset_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) tracking_mode: String,
    pub(super) serial_number: Option<String>,
    pub(super) batch_number: Option<String>,
    pub(super) quantity_on_hand: f64,
    pub(super) quantity_allocated: f64,
    pub(super) quantity_unit_id: Uuid,
    pub(super) location_id: Option<Uuid>,
    pub(super) status: String,
    pub(super) public_notes: Option<String>,
    pub(super) internal_notes: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) last_stocktake_at: Option<DateTime<Utc>>,
    pub(super) asset_category_id: Option<Uuid>,
    pub(super) asset_name: String,
    pub(super) asset_model: Option<String>,
    pub(super) asset_manufacturer: Option<String>,
    pub(super) asset_default_unit_id: Uuid,
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

impl From<InventoryItemRow> for InventoryItemResponse {
    fn from(row: InventoryItemRow) -> Self {
        Self::from_row(row, true)
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

#[derive(Clone)]
pub(super) struct InventoryItemPatch {
    pub(super) serial_number: Option<String>,
    pub(super) batch_number: NullableUpdate<String>,
    pub(super) quantity_on_hand: Option<f64>,
    pub(super) quantity_allocated: Option<f64>,
    pub(super) quantity_unit_id: Option<Uuid>,
    pub(super) location_id: NullableUpdate<Uuid>,
    pub(super) status: Option<String>,
    pub(super) public_notes: NullableUpdate<String>,
    pub(super) internal_notes: NullableUpdate<String>,
}

#[derive(Clone, sqlx::FromRow)]
struct UnitRow {
    dimension: String,
    scale_to_base: f64,
}

pub(super) async fn actor_for_user(
    pool: &PgPool,
    actor_user_id: UserId,
) -> Result<Actor, InventoryItemError> {
    get_actor(pool, actor_user_id)
        .await
        .map_err(InventoryItemError::UnexpectedError)?
        .ok_or_else(|| InventoryItemError::Forbidden("Actor not found in the database".into()))
}

pub(super) fn validate_read_permission(
    actor: &Actor,
    laboratory_id: Uuid,
) -> Result<LaboratoryId, InventoryItemError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id)
        .map_err(|e| InventoryItemError::UnexpectedError(anyhow!("{e}")))?;
    if actor.can_query_laboratory_resource(&laboratory_id) {
        Ok(laboratory_id)
    } else {
        Err(InventoryItemError::Forbidden(
            "You do not have permission to view inventory items for this laboratory".into(),
        ))
    }
}

pub(super) fn validate_write_permission(
    actor: &Actor,
    laboratory_id: Uuid,
) -> Result<LaboratoryId, InventoryItemError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id)
        .map_err(|e| InventoryItemError::UnexpectedError(anyhow!("{e}")))?;
    if actor.can_write_laboratory_resource(&laboratory_id) {
        Ok(laboratory_id)
    } else {
        Err(InventoryItemError::Forbidden(
            "You don't have permission to change inventory items for this laboratory".into(),
        ))
    }
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
) -> Result<Option<InventoryItemRow>, InventoryItemError> {
    let query = format!(
        "{} WHERE asset_inventory_items.inventory_item_id = $1",
        inventory_item_select()
    );
    sqlx::query_as::<_, InventoryItemRow>(&query)
        .bind(inventory_item_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| InventoryItemError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_inventory_item_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
) -> Result<Option<InventoryItemRow>, InventoryItemError> {
    let query = format!(
        "{} WHERE asset_inventory_items.inventory_item_id = $1 FOR UPDATE OF asset_inventory_items",
        inventory_item_select()
    );
    sqlx::query_as::<_, InventoryItemRow>(&query)
        .bind(inventory_item_id)
        .fetch_optional(transaction.as_mut())
        .await
        .map_err(|e| InventoryItemError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_inventory_items_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_ids: &[Uuid],
) -> Result<Vec<InventoryItemRow>, InventoryItemError> {
    let query = format!(
        "{} WHERE asset_inventory_items.inventory_item_id = ANY($1) ORDER BY asset_inventory_items.inventory_item_id FOR UPDATE OF asset_inventory_items",
        inventory_item_select()
    );
    sqlx::query_as::<_, InventoryItemRow>(&query)
        .bind(inventory_item_ids)
        .fetch_all(transaction.as_mut())
        .await
        .map_err(|e| InventoryItemError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_asset_for_inventory_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
) -> Result<Option<AssetForInventoryRow>, InventoryItemError> {
    sqlx::query_as::<_, AssetForInventoryRow>(
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
    )
    .bind(asset_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| InventoryItemError::UnexpectedError(e.into()))
}

pub(super) fn validate_requested_ids(
    requested_ids: &[Uuid],
    rows: &[InventoryItemRow],
) -> Result<(), InventoryItemError> {
    if requested_ids.is_empty() {
        return Err(InventoryItemError::ValidationError(
            "inventory_item_ids cannot be empty".into(),
        ));
    }
    let unique_ids: HashSet<_> = requested_ids.iter().copied().collect();
    if unique_ids.len() != requested_ids.len() {
        return Err(InventoryItemError::ValidationError(
            "inventory_item_ids cannot contain duplicates".into(),
        ));
    }
    if rows.len() != requested_ids.len() {
        return Err(InventoryItemError::NotFound(
            "One or more inventory items were not found".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_status(
    status: Option<String>,
) -> Result<Option<String>, InventoryItemError> {
    status
        .map(|status| {
            AssetInventoryStatus::parse(&status)
                .map(|status| status.as_str().to_string())
                .map_err(InventoryItemError::ValidationError)
        })
        .transpose()
}

pub(super) fn parse_nullable_string(value: Option<Option<String>>) -> NullableUpdate<String> {
    match value {
        Some(Some(value)) => empty_to_nullable_update(value),
        Some(None) => NullableUpdate::Clear,
        None => NullableUpdate::Unchanged,
    }
}

pub(super) fn parse_nullable_uuid(value: Option<Option<Uuid>>) -> NullableUpdate<Uuid> {
    match value {
        Some(Some(value)) => NullableUpdate::Set(value),
        Some(None) => NullableUpdate::Clear,
        None => NullableUpdate::Unchanged,
    }
}

pub(super) async fn validate_location(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    location_id: Uuid,
) -> Result<(), InventoryItemError> {
    let found: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT location_id
        FROM locations
        WHERE laboratory_id = $1
          AND location_id = $2
        FOR UPDATE
        "#,
    )
    .bind(laboratory_id)
    .bind(location_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| InventoryItemError::UnexpectedError(e.into()))?;

    if found.is_some() {
        Ok(())
    } else {
        Err(InventoryItemError::ValidationError(
            "Inventory item location does not belong to this laboratory".into(),
        ))
    }
}

pub(super) async fn next_serial_numbers(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    count: i64,
) -> Result<Vec<String>, InventoryItemError> {
    if count <= 0 {
        return Err(InventoryItemError::ValidationError(
            "count must be positive".into(),
        ));
    }
    if count > 200 {
        return Err(InventoryItemError::ValidationError(
            "count cannot exceed 200".into(),
        ));
    }

    let max_serial: i32 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(MAX(substring(serial_number from 2)::integer), 0)
        FROM asset_inventory_items
        WHERE asset_id = $1
          AND serial_number ~ '^#[0-9]+$'
        "#,
    )
    .bind(asset_id)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(|e| InventoryItemError::UnexpectedError(e.into()))?;

    Ok((1..=count)
        .map(|offset| format!("#{}", max_serial as i64 + offset))
        .collect())
}

pub(super) fn normalize_serial_numbers(
    serial_numbers: Vec<String>,
) -> Result<Vec<String>, InventoryItemError> {
    if serial_numbers.is_empty() {
        return Err(InventoryItemError::ValidationError(
            "serial_numbers cannot be empty".into(),
        ));
    }
    if serial_numbers.len() > 200 {
        return Err(InventoryItemError::ValidationError(
            "serial_numbers cannot contain more than 200 values".into(),
        ));
    }

    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(serial_numbers.len());
    for serial_number in serial_numbers {
        let serial_number = serial_number.trim().to_string();
        if serial_number.is_empty() {
            return Err(InventoryItemError::ValidationError(
                "serial_numbers cannot contain blank values".into(),
            ));
        }
        if !seen.insert(serial_number.clone()) {
            return Err(InventoryItemError::ValidationError(
                "serial_numbers cannot contain duplicates".into(),
            ));
        }
        normalized.push(serial_number);
    }
    Ok(normalized)
}

#[allow(clippy::too_many_arguments)]
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
) -> Result<InventoryItemRow, InventoryItemError> {
    let inventory_item_id = Uuid::new_v4();
    sqlx::query(
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
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(inventory_item_id)
    .bind(asset_id)
    .bind(laboratory_id)
    .bind(tracking_mode)
    .bind(trim_optional(serial_number))
    .bind(trim_optional(batch_number))
    .bind(quantity_on_hand)
    .bind(quantity_allocated)
    .bind(quantity_unit_id)
    .bind(location_id)
    .bind(status)
    .bind(trim_optional(public_notes))
    .bind(trim_optional(internal_notes))
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    fetch_inventory_item_for_update(transaction, inventory_item_id)
        .await?
        .ok_or_else(|| {
            InventoryItemError::UnexpectedError(anyhow!("Created inventory item not found"))
        })
}

pub(super) async fn apply_inventory_item_patch(
    transaction: &mut Transaction<'_, Postgres>,
    existing: &InventoryItemRow,
    patch: InventoryItemPatch,
) -> Result<InventoryItemRow, InventoryItemError> {
    let tracking_mode = AssetTrackingMode::parse(&existing.tracking_mode)
        .map_err(InventoryItemError::ValidationError)?;

    match tracking_mode {
        AssetTrackingMode::Serialized => {
            if patch.quantity_on_hand.is_some()
                || patch.quantity_allocated.is_some()
                || patch.quantity_unit_id.is_some()
            {
                return Err(InventoryItemError::ValidationError(
                    "Serialized inventory items cannot update quantity fields".into(),
                ));
            }
        }
        AssetTrackingMode::Quantity => {
            if patch.serial_number.is_some() {
                return Err(InventoryItemError::ValidationError(
                    "Quantity-tracked inventory items cannot update serial_number".into(),
                ));
            }
        }
    }

    let serial_number = match patch.serial_number {
        Some(serial_number) => {
            let serial_number = serial_number.trim().to_string();
            if serial_number.is_empty() {
                return Err(InventoryItemError::ValidationError(
                    "serial_number cannot be blank".into(),
                ));
            }
            Some(serial_number)
        }
        None => existing.serial_number.clone(),
    };
    let batch_number = patch
        .batch_number
        .resolve(existing.batch_number.clone())
        .and_then(|value| trim_optional(Some(&value)));
    let location_id = patch.location_id.resolve(existing.location_id);
    if let Some(location_id) = location_id {
        validate_location(transaction, existing.laboratory_id, location_id).await?;
    }
    let status = validate_status(patch.status)?.unwrap_or_else(|| existing.status.clone());
    let mut quantity_on_hand = patch.quantity_on_hand.unwrap_or(existing.quantity_on_hand);
    let mut quantity_allocated = patch
        .quantity_allocated
        .unwrap_or(existing.quantity_allocated);
    let quantity_unit_id = match tracking_mode {
        AssetTrackingMode::Serialized => existing.quantity_unit_id,
        AssetTrackingMode::Quantity => {
            resolve_asset_quantity_unit(patch.quantity_unit_id, existing.asset_default_unit_id)?
        }
    };
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
    let public_notes = patch
        .public_notes
        .resolve(existing.public_notes.clone())
        .and_then(|value| trim_optional(Some(&value)));
    let internal_notes = patch
        .internal_notes
        .resolve(existing.internal_notes.clone())
        .and_then(|value| trim_optional(Some(&value)));

    validate_quantities(quantity_on_hand, quantity_allocated)?;

    sqlx::query(
        r#"
        UPDATE asset_inventory_items
        SET
            serial_number = $2,
            batch_number = $3,
            quantity_on_hand = $4,
            quantity_allocated = $5,
            quantity_unit_id = $6,
            location_id = $7,
            status = $8,
            public_notes = $9,
            internal_notes = $10,
            updated_at = now()
        WHERE inventory_item_id = $1
        "#,
    )
    .bind(existing.inventory_item_id)
    .bind(serial_number.as_deref())
    .bind(batch_number.as_deref())
    .bind(quantity_on_hand)
    .bind(quantity_allocated)
    .bind(quantity_unit_id)
    .bind(location_id)
    .bind(&status)
    .bind(public_notes.as_deref())
    .bind(internal_notes.as_deref())
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    fetch_inventory_item_for_update(transaction, existing.inventory_item_id)
        .await?
        .ok_or_else(|| {
            InventoryItemError::UnexpectedError(anyhow!("Updated inventory item not found"))
        })
}

pub(super) async fn set_quantity_on_hand(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    quantity_on_hand: f64,
) -> Result<InventoryItemRow, InventoryItemError> {
    sqlx::query(
        r#"
        UPDATE asset_inventory_items
        SET quantity_on_hand = $2, updated_at = now()
        WHERE inventory_item_id = $1
        "#,
    )
    .bind(inventory_item_id)
    .bind(quantity_on_hand)
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;
    fetch_inventory_item_for_update(transaction, inventory_item_id)
        .await?
        .ok_or_else(|| {
            InventoryItemError::UnexpectedError(anyhow!("Updated inventory item not found"))
        })
}

pub(super) async fn add_quantities_to_item(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    quantity_delta: f64,
    allocated_delta: f64,
) -> Result<InventoryItemRow, InventoryItemError> {
    sqlx::query(
        r#"
        UPDATE asset_inventory_items
        SET
            quantity_on_hand = quantity_on_hand + $2,
            quantity_allocated = quantity_allocated + $3,
            updated_at = now()
        WHERE inventory_item_id = $1
        "#,
    )
    .bind(inventory_item_id)
    .bind(quantity_delta)
    .bind(allocated_delta)
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;
    fetch_inventory_item_for_update(transaction, inventory_item_id)
        .await?
        .ok_or_else(|| {
            InventoryItemError::UnexpectedError(anyhow!("Updated inventory item not found"))
        })
}

pub(super) async fn find_quantity_aggregate_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    asset_id: Uuid,
    batch_number: Option<&str>,
    location_id: Option<Uuid>,
    status: &str,
    quantity_unit_id: Uuid,
    exclude_inventory_item_id: Option<Uuid>,
) -> Result<Option<InventoryItemRow>, InventoryItemError> {
    let query = format!(
        r#"
        {}
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
        inventory_item_select()
    );
    sqlx::query_as::<_, InventoryItemRow>(&query)
        .bind(laboratory_id)
        .bind(asset_id)
        .bind(batch_number)
        .bind(location_id)
        .bind(status)
        .bind(quantity_unit_id)
        .bind(exclude_inventory_item_id)
        .fetch_optional(transaction.as_mut())
        .await
        .map_err(|e| InventoryItemError::UnexpectedError(e.into()))
}

pub(super) async fn delete_inventory_item_attachments(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
) -> Result<Vec<DeletedAttachmentRow>, InventoryItemError> {
    let rows = sqlx::query_as::<_, DeletedAttachmentRow>(
        r#"
        WITH updated AS (
            UPDATE attachments
            SET deleted_at = now(),
                updated_at = now()
            WHERE inventory_item_id = $1
              AND deleted_at IS NULL
            RETURNING attachment_id, storage_key
        )
        SELECT attachment_id, storage_key
        FROM updated
        ORDER BY attachment_id
        "#,
    )
    .bind(inventory_item_id)
    .fetch_all(transaction.as_mut())
    .await
    .map_err(|e| InventoryItemError::UnexpectedError(e.into()))?;
    Ok(rows)
}

pub(super) async fn move_inventory_item_attachments(
    transaction: &mut Transaction<'_, Postgres>,
    from_inventory_item_ids: &[Uuid],
    to_inventory_item_id: Uuid,
) -> Result<Vec<Uuid>, InventoryItemError> {
    let attachment_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        UPDATE attachments
        SET inventory_item_id = $2,
            updated_at = now()
        WHERE inventory_item_id = ANY($1)
          AND deleted_at IS NULL
        RETURNING attachment_id
        "#,
    )
    .bind(from_inventory_item_ids)
    .bind(to_inventory_item_id)
    .fetch_all(transaction.as_mut())
    .await
    .map_err(|e| InventoryItemError::UnexpectedError(e.into()))?;
    Ok(attachment_ids)
}

pub(super) async fn delete_inventory_item_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
) -> Result<(), InventoryItemError> {
    sqlx::query("DELETE FROM asset_inventory_items WHERE inventory_item_id = $1")
        .bind(inventory_item_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
    Ok(())
}

pub(super) fn validate_quantity_item(row: &InventoryItemRow) -> Result<(), InventoryItemError> {
    if row.tracking_mode == "quantity" {
        Ok(())
    } else {
        Err(InventoryItemError::ValidationError(
            "Operation only applies to quantity-tracked inventory items".into(),
        ))
    }
}

pub(super) fn validate_quantities(
    quantity_on_hand: f64,
    quantity_allocated: f64,
) -> Result<(), InventoryItemError> {
    if quantity_on_hand < 0.0 {
        return Err(InventoryItemError::ValidationError(
            "quantity_on_hand must be non-negative".into(),
        ));
    }
    if quantity_allocated < 0.0 {
        return Err(InventoryItemError::ValidationError(
            "quantity_allocated must be non-negative".into(),
        ));
    }
    if quantity_allocated > quantity_on_hand {
        return Err(InventoryItemError::ValidationError(
            "quantity_allocated cannot exceed quantity_on_hand".into(),
        ));
    }
    Ok(())
}

pub(super) fn resolve_asset_quantity_unit(
    requested_unit_id: Option<Uuid>,
    asset_default_unit_id: Uuid,
) -> Result<Uuid, InventoryItemError> {
    if requested_unit_id.is_some_and(|unit_id| unit_id != asset_default_unit_id) {
        return Err(InventoryItemError::ValidationError(
            "Inventory item unit must match asset default unit".into(),
        ));
    }
    Ok(asset_default_unit_id)
}

async fn fetch_unit_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> Result<UnitRow, InventoryItemError> {
    sqlx::query_as::<_, UnitRow>(
        r#"
        SELECT dimension, scale_to_base
        FROM units
        WHERE unit_id = $1
        FOR UPDATE
        "#,
    )
    .bind(unit_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| InventoryItemError::UnexpectedError(e.into()))?
    .ok_or_else(|| InventoryItemError::ValidationError("Unit not found".into()))
}

pub(super) async fn convert_quantity_between_units(
    transaction: &mut Transaction<'_, Postgres>,
    source_unit_id: Uuid,
    target_unit_id: Uuid,
    source_quantity: f64,
) -> Result<f64, InventoryItemError> {
    if source_unit_id == target_unit_id {
        return Ok(source_quantity);
    }
    let source_unit = fetch_unit_for_update(transaction, source_unit_id).await?;
    let target_unit = fetch_unit_for_update(transaction, target_unit_id).await?;
    if source_unit.dimension != target_unit.dimension {
        return Err(InventoryItemError::ValidationError(
            "Target unit dimension does not match source unit dimension".into(),
        ));
    }
    Ok(source_quantity * source_unit.scale_to_base / target_unit.scale_to_base)
}

pub(super) async fn record_inventory_item_audit(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    action: AuditAction,
    inventory_item_id: Uuid,
    details: Value,
) -> Result<(), InventoryItemError> {
    record_audit(
        transaction,
        actor,
        action,
        AuditResource::InventoryItem,
        Some(inventory_item_id),
        details,
    )
    .await
    .map_err(InventoryItemError::UnexpectedError)
}

pub(super) async fn record_inventory_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    row: &InventoryItemRow,
    action: &str,
    quantity_delta: f64,
    allocated_delta: f64,
    from_location_id: Option<Uuid>,
    to_location_id: Option<Uuid>,
    details: Value,
) -> Result<(), InventoryItemError> {
    let actor_laboratory_id = actor.laboratory_id.map(|laboratory_id| *laboratory_id);
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
            details
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(row.inventory_item_id)
    .bind(row.laboratory_id)
    .bind(*actor.user_id)
    .bind(actor_laboratory_id)
    .bind(action)
    .bind(quantity_delta)
    .bind(allocated_delta)
    .bind(from_location_id)
    .bind(to_location_id)
    .bind(details)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| InventoryItemError::UnexpectedError(e.into()))?;
    Ok(())
}

pub(super) async fn record_update_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    before: &InventoryItemRow,
    after: &InventoryItemRow,
    operation: &str,
) -> Result<(), InventoryItemError> {
    let quantity_delta = after.quantity_on_hand - before.quantity_on_hand;
    let allocated_delta = after.quantity_allocated - before.quantity_allocated;
    let location_changed = before.location_id != after.location_id;
    let action = if location_changed {
        "move"
    } else if quantity_delta.abs() > f64::EPSILON {
        "adjust"
    } else if allocated_delta > f64::EPSILON {
        "allocate"
    } else if allocated_delta < -f64::EPSILON {
        "release_allocation"
    } else {
        "update"
    };

    record_inventory_transaction(
        transaction,
        actor,
        after,
        action,
        quantity_delta,
        allocated_delta,
        before.location_id,
        after.location_id,
        json!({
            "operation": operation,
            "before": before,
            "after": after,
        }),
    )
    .await
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

pub(super) fn update_inventory_item_rollback_details(item: &InventoryItemRow) -> Value {
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

fn map_database_error(error: sqlx::Error) -> InventoryItemError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("idx_asset_inventory_items_unique_asset_serial_number")) => {
                return InventoryItemError::ConflictError(
                    "Inventory item serial number already exists for this asset".into(),
                );
            }
            (Some("23505"), Some("idx_asset_inventory_items_unique_quantity_aggregate")) => {
                return InventoryItemError::ConflictError(
                    "Inventory item already exists for this quantity aggregate".into(),
                );
            }
            (Some("23505"), _) => {
                return InventoryItemError::ConflictError("Inventory item already exists".into());
            }
            (Some("23503"), _) => {
                return InventoryItemError::ValidationError("Invalid referenced record".into());
            }
            (Some("23514"), _) => {
                return InventoryItemError::ValidationError("Invalid inventory item data".into());
            }
            _ => {}
        }
    }
    InventoryItemError::UnexpectedError(error.into())
}

fn trim_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn empty_to_nullable_update(value: String) -> NullableUpdate<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        NullableUpdate::Clear
    } else {
        NullableUpdate::Set(value)
    }
}
