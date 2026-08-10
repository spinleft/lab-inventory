//! Every SQL statement the inventory item routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - functions never return a handler error type, only
//!   [`InventoryItemDatabaseError`], so any handler can reuse them
use super::model::{AssetForInventoryRow, DeletedAttachmentRow, InventoryItemRow};
use crate::utils::error_chain_fmt;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(crate) enum InventoryItemDatabaseError {
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

/// The projection shared with [`fetch_inventory_item`], for `QueryBuilder`
/// callers that need to append their own filters.
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
        assets.inventory_unit_id AS asset_inventory_unit_id
    FROM asset_inventory_items
    JOIN assets
      ON assets.asset_id = asset_inventory_items.asset_id
    "#
}

// ---------------------------------------------------------------------------
// inventory items
// ---------------------------------------------------------------------------

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
            assets.inventory_unit_id AS asset_inventory_unit_id
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

/// Same projection as [`fetch_inventory_item`], but takes the row lock the write
/// paths need. Only the item itself is locked: the asset is joined for display
/// and must stay free for other writers.
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
            assets.inventory_unit_id AS asset_inventory_unit_id
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

/// Locks a whole batch in id order, so two concurrent batch operations touching
/// overlapping sets cannot deadlock against each other.
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
            assets.inventory_unit_id AS asset_inventory_unit_id
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

/// The quantity-tracked row that already holds this exact combination of batch,
/// location and status, if there is one. A split merges into it rather than
/// creating a duplicate; `exclude_inventory_item_id` keeps the row being split
/// from matching itself.
#[allow(clippy::too_many_arguments)]
pub(super) async fn find_quantity_aggregate_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    asset_id: Uuid,
    batch_number: Option<&str>,
    location_id: Option<Uuid>,
    status: &str,
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
            assets.inventory_unit_id AS asset_inventory_unit_id
        FROM asset_inventory_items
        JOIN assets
          ON assets.asset_id = asset_inventory_items.asset_id
        WHERE asset_inventory_items.tracking_mode = 'quantity'
          AND asset_inventory_items.laboratory_id = $1
          AND asset_inventory_items.asset_id = $2
          AND asset_inventory_items.batch_number IS NOT DISTINCT FROM $3
          AND asset_inventory_items.location_id IS NOT DISTINCT FROM $4
          AND asset_inventory_items.status = $5
          AND ($6::uuid IS NULL OR asset_inventory_items.inventory_item_id <> $6)
        FOR UPDATE OF asset_inventory_items
        "#,
        laboratory_id,
        asset_id,
        batch_number,
        location_id,
        status,
        exclude_inventory_item_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch quantity aggregate for update")
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
    inventory_item_id: Uuid,
    asset_id: Uuid,
    laboratory_id: Uuid,
    tracking_mode: &str,
    serial_number: Option<&str>,
    batch_number: Option<&str>,
    quantity_on_hand: f64,
    quantity_allocated: f64,
    location_id: Option<Uuid>,
    status: &str,
    public_notes: Option<&str>,
    internal_notes: Option<&str>,
) -> Result<(), InventoryItemDatabaseError> {
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
            location_id,
            status,
            public_notes,
            internal_notes
        )
        VALUES (
            $1, $2, $3, $4, $5, $6,
            $7::double precision::numeric,
            $8::double precision::numeric,
            $9, $10, $11, $12
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
        location_id,
        status,
        trim_optional(public_notes),
        trim_optional(internal_notes),
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Updating inventory item in the database",
    skip(
        transaction,
        serial_number,
        batch_number,
        status,
        public_notes,
        internal_notes
    ),
    fields(inventory_item_id=%inventory_item_id)
)]
pub(super) async fn update_inventory_item_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    serial_number: Option<&str>,
    batch_number: Option<&str>,
    quantity_on_hand: f64,
    quantity_allocated: f64,
    location_id: Option<Uuid>,
    status: &str,
    public_notes: Option<&str>,
    internal_notes: Option<&str>,
) -> Result<(), InventoryItemDatabaseError> {
    sqlx::query!(
        r#"
        UPDATE asset_inventory_items
        SET
            serial_number = $2,
            batch_number = $3,
            quantity_on_hand = $4::double precision::numeric,
            quantity_allocated = $5::double precision::numeric,
            location_id = $6,
            status = $7,
            public_notes = $8,
            internal_notes = $9,
            updated_at = now()
        WHERE inventory_item_id = $1
        "#,
        inventory_item_id,
        serial_number,
        batch_number,
        quantity_on_hand,
        quantity_allocated,
        location_id,
        status,
        public_notes,
        internal_notes,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

#[tracing::instrument(
    name = "Updating inventory item quantity in the database",
    skip(transaction),
    fields(inventory_item_id=%inventory_item_id)
)]
pub(super) async fn set_quantity_on_hand_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    quantity_on_hand: f64,
) -> Result<(), InventoryItemDatabaseError> {
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

    Ok(())
}

/// Adds to the stored quantities rather than overwriting them, so the value the
/// caller read cannot go stale between the read and the write.
#[tracing::instrument(
    name = "Adding inventory item quantities in the database",
    skip(transaction),
    fields(inventory_item_id=%inventory_item_id)
)]
pub(super) async fn add_quantities_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    quantity_delta: f64,
    allocated_delta: f64,
) -> Result<(), InventoryItemDatabaseError> {
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

    Ok(())
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

/// The highest `#N` serial number already used for this asset, or 0 when the
/// asset has none. Serial numbers set by hand in another format are ignored.
pub(super) async fn max_generated_serial_number(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
) -> Result<i32, anyhow::Error> {
    sqlx::query_scalar!(
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
    .context("Failed to fetch the next inventory item serial number")
}

// ---------------------------------------------------------------------------
// existence checks and lookups
// ---------------------------------------------------------------------------

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
            tracking_mode
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

// ---------------------------------------------------------------------------
// attachments
// ---------------------------------------------------------------------------

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

/// A merge folds the source items into the target, so their attachments have to
/// follow rather than disappear with the rows.
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

fn trim_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
