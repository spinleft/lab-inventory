use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
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
    pub(crate) asset_inventory_unit_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct InventoryItemAssetResponse {
    asset_id: Uuid,
    category_id: Option<Uuid>,
    name: String,
    model: Option<String>,
    manufacturer: Option<String>,
    inventory_unit_id: Uuid,
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
    location_id: Option<Uuid>,
    status: String,
    public_notes: Option<String>,
    internal_notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_stocktake_at: Option<DateTime<Utc>>,
    asset: InventoryItemAssetResponse,
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
                inventory_unit_id: row.asset_inventory_unit_id,
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
