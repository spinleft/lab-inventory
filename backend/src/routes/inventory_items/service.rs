//! Business flows that chain several statements together, and the rules those
//! flows enforce.
//!
//! Anything here orchestrates `queries.rs`. Single-statement work belongs in
//! `queries.rs`; HTTP concerns belong in the handler modules.
use super::model::InventoryItemRow;
use super::queries::{
    InventoryItemDatabaseError, add_quantities_in_database, fetch_inventory_item_for_update,
    insert_inventory_item as insert_inventory_item_row, max_generated_serial_number,
    set_quantity_on_hand_in_database, update_inventory_item_in_database, validate_location,
};
use crate::domain::{AssetTrackingMode, InventoryStatus, UpdateInventoryItem};
use anyhow::anyhow;
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// validation
// ---------------------------------------------------------------------------

/// Batch operations name their items by id, so the list has to be a real set of
/// items that all exist before any of them is touched.
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

/// Splitting and merging move amounts around, which only means something for
/// items counted by quantity: a serialized item is one physical thing.
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

fn validate_patch_against_tracking_mode(
    patch: &UpdateInventoryItem,
    tracking_mode: AssetTrackingMode,
) -> Result<(), InventoryItemDatabaseError> {
    match tracking_mode {
        AssetTrackingMode::Serialized => {
            if patch.quantity_on_hand.is_some() || patch.quantity_allocated.is_some() {
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

// ---------------------------------------------------------------------------
// writes
// ---------------------------------------------------------------------------

/// The serial numbers the next `count` generated items will carry, continuing
/// the `#N` sequence the asset already uses.
pub(super) async fn next_serial_numbers(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    count: u16,
) -> Result<Vec<String>, anyhow::Error> {
    let max_serial = max_generated_serial_number(transaction, asset_id).await?;

    Ok((1..=count)
        .map(|offset| format!("#{}", i64::from(max_serial) + i64::from(offset)))
        .collect())
}

/// Writes one inventory item and reads it back, so the caller gets the row as
/// the database sees it — including the asset columns the response carries.
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
    location_id: Option<Uuid>,
    status: &str,
    public_notes: Option<&str>,
    internal_notes: Option<&str>,
) -> Result<InventoryItemRow, InventoryItemDatabaseError> {
    let inventory_item_id = Uuid::new_v4();
    insert_inventory_item_row(
        transaction,
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
        internal_notes,
    )
    .await?;

    reread_inventory_item(transaction, inventory_item_id, "Created").await
}

/// Applies a patch to one item: fields the patch leaves out keep the value they
/// already had, and what the tracking mode forbids is rejected before anything
/// is written.
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
    let quantity_on_hand = patch.quantity_on_hand.unwrap_or(existing.quantity_on_hand);
    let quantity_allocated = patch
        .quantity_allocated
        .unwrap_or(existing.quantity_allocated);
    let public_notes = patch.public_notes.resolve(existing.public_notes.clone());
    let internal_notes = patch
        .internal_notes
        .resolve(existing.internal_notes.clone());
    validate_quantities(quantity_on_hand, quantity_allocated)
        .map_err(InventoryItemDatabaseError::Validation)?;

    update_inventory_item_in_database(
        transaction,
        existing.inventory_item_id,
        serial_number.as_deref(),
        batch_number.as_deref(),
        quantity_on_hand,
        quantity_allocated,
        location_id,
        &status,
        public_notes.as_deref(),
        internal_notes.as_deref(),
    )
    .await?;

    reread_inventory_item(transaction, existing.inventory_item_id, "Updated").await
}

pub(super) async fn set_quantity_on_hand(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    quantity_on_hand: f64,
) -> Result<InventoryItemRow, InventoryItemDatabaseError> {
    set_quantity_on_hand_in_database(transaction, inventory_item_id, quantity_on_hand).await?;

    reread_inventory_item(transaction, inventory_item_id, "Updated").await
}

pub(super) async fn add_quantities_to_item(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    quantity_delta: f64,
    allocated_delta: f64,
) -> Result<InventoryItemRow, InventoryItemDatabaseError> {
    add_quantities_in_database(
        transaction,
        inventory_item_id,
        quantity_delta,
        allocated_delta,
    )
    .await?;

    reread_inventory_item(transaction, inventory_item_id, "Updated").await
}

/// Reads back a row this transaction just wrote. Not finding it would mean the
/// write did not take, which is a bug rather than a missing record.
async fn reread_inventory_item(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    what: &str,
) -> Result<InventoryItemRow, InventoryItemDatabaseError> {
    fetch_inventory_item_for_update(transaction, inventory_item_id)
        .await?
        .ok_or(InventoryItemDatabaseError::Unexpected(anyhow!(
            "{what} inventory item not found"
        )))
}
