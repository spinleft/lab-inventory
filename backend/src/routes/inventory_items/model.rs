use crate::authentication::Actor;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct InventoryItemRow {
    pub inventory_item_id: Uuid,
    pub asset_id: Uuid,
    pub asset_name: String,
    pub asset_model: Option<String>,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub tracking_mode: String,
    pub serial_number: Option<String>,
    pub batch_number: Option<String>,
    pub quantity_on_hand: f64,
    pub quantity_allocated: f64,
    pub unit_id: Uuid,
    pub unit_code: String,
    pub unit_allow_decimal: bool,
    pub location_id: Option<Uuid>,
    pub location_name: Option<String>,
    pub status: String,
    pub is_cross_lab_borrowable: bool,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct InventoryItemResponse {
    pub inventory_item_id: Uuid,
    pub asset_id: Uuid,
    pub asset_name: String,
    pub asset_model: Option<String>,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub tracking_mode: String,
    pub serial_number: Option<String>,
    pub batch_number: Option<String>,
    pub quantity_on_hand: f64,
    pub quantity_allocated: f64,
    pub quantity_available: f64,
    pub unit_id: Uuid,
    pub unit_code: String,
    pub location_id: Option<Uuid>,
    pub location_name: Option<String>,
    pub status: String,
    pub is_cross_lab_borrowable: bool,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl InventoryItemResponse {
    pub fn from_row(row: InventoryItemRow, actor: &Actor) -> Self {
        let show_sensitive = actor.is_system_admin() || actor.is_same_laboratory(row.laboratory_id);
        Self {
            inventory_item_id: row.inventory_item_id,
            asset_id: row.asset_id,
            asset_name: row.asset_name,
            asset_model: row.asset_model,
            laboratory_id: row.laboratory_id,
            laboratory_name: row.laboratory_name,
            tracking_mode: row.tracking_mode,
            serial_number: show_sensitive.then_some(row.serial_number).flatten(),
            batch_number: show_sensitive.then_some(row.batch_number).flatten(),
            quantity_on_hand: row.quantity_on_hand,
            quantity_allocated: row.quantity_allocated,
            quantity_available: row.quantity_on_hand - row.quantity_allocated,
            unit_id: row.unit_id,
            unit_code: row.unit_code,
            location_id: show_sensitive.then_some(row.location_id).flatten(),
            location_name: show_sensitive.then_some(row.location_name).flatten(),
            status: row.status,
            is_cross_lab_borrowable: row.is_cross_lab_borrowable,
            public_notes: row.public_notes,
            internal_notes: show_sensitive.then_some(row.internal_notes).flatten(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
pub struct AssetForInventory {
    pub asset_id: Uuid,
    pub laboratory_id: Uuid,
    pub tracking_mode: String,
    pub default_unit_id: Uuid,
}
