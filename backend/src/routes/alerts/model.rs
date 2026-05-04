use crate::authentication::Actor;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(super) struct StockAlertRow {
    pub asset_id: Uuid,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub asset_kind: String,
    pub tracking_mode: String,
    pub name: String,
    pub model: Option<String>,
    pub default_unit_id: Uuid,
    pub default_unit_code: String,
    pub minimum_stock_quantity: f64,
    pub minimum_stock_unit_id: Uuid,
    pub minimum_stock_unit_code: String,
    pub quantity_available: f64,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
}

#[derive(Serialize)]
pub(super) struct StockAlertResponse {
    pub asset_id: Uuid,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub asset_kind: String,
    pub tracking_mode: String,
    pub name: String,
    pub model: Option<String>,
    pub default_unit_id: Uuid,
    pub default_unit_code: String,
    pub minimum_stock_quantity: f64,
    pub minimum_stock_unit_id: Uuid,
    pub minimum_stock_unit_code: String,
    pub quantity_available: f64,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
}

impl StockAlertResponse {
    pub fn from_row(row: StockAlertRow, actor: &Actor) -> Self {
        let show_sensitive = actor.is_system_admin() || actor.is_same_laboratory(row.laboratory_id);
        Self {
            asset_id: row.asset_id,
            laboratory_id: row.laboratory_id,
            laboratory_name: row.laboratory_name,
            category_id: row.category_id,
            category_name: row.category_name,
            asset_kind: row.asset_kind,
            tracking_mode: row.tracking_mode,
            name: row.name,
            model: row.model,
            default_unit_id: row.default_unit_id,
            default_unit_code: row.default_unit_code,
            minimum_stock_quantity: row.minimum_stock_quantity,
            minimum_stock_unit_id: row.minimum_stock_unit_id,
            minimum_stock_unit_code: row.minimum_stock_unit_code,
            quantity_available: row.quantity_available,
            public_notes: row.public_notes,
            internal_notes: show_sensitive.then_some(row.internal_notes).flatten(),
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct BorrowRequestAlert {
    pub borrow_request_id: Uuid,
    pub alert_kind: String,
    pub inventory_item_id: Uuid,
    pub asset_id: Uuid,
    pub asset_name: String,
    pub asset_model: Option<String>,
    pub requester_user_id: Uuid,
    pub requester_username: String,
    pub requester_laboratory_id: Uuid,
    pub requester_laboratory_name: String,
    pub owner_laboratory_id: Uuid,
    pub owner_laboratory_name: String,
    pub requested_quantity: f64,
    pub unit_id: Uuid,
    pub unit_code: String,
    pub expected_borrowed_at: Option<DateTime<Utc>>,
    pub expected_returned_at: Option<DateTime<Utc>>,
    pub purpose: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub(super) struct MaintenanceAlertRow {
    pub maintenance_schedule_id: Uuid,
    pub alert_kind: String,
    pub asset_id: Option<Uuid>,
    pub inventory_item_id: Option<Uuid>,
    pub asset_name: String,
    pub asset_model: Option<String>,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub schedule_name: String,
    pub interval_days: i32,
    pub next_maintenance_at: DateTime<Utc>,
    pub remind_before_days: i32,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
}

#[derive(Serialize)]
pub(super) struct MaintenanceAlertResponse {
    pub maintenance_schedule_id: Uuid,
    pub alert_kind: String,
    pub asset_id: Option<Uuid>,
    pub inventory_item_id: Option<Uuid>,
    pub asset_name: String,
    pub asset_model: Option<String>,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub schedule_name: String,
    pub interval_days: i32,
    pub next_maintenance_at: DateTime<Utc>,
    pub remind_before_days: i32,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
}

impl MaintenanceAlertResponse {
    pub fn from_row(row: MaintenanceAlertRow, actor: &crate::authentication::Actor) -> Self {
        let show_sensitive = actor.is_system_admin() || actor.is_same_laboratory(row.laboratory_id);
        Self {
            maintenance_schedule_id: row.maintenance_schedule_id,
            alert_kind: row.alert_kind,
            asset_id: row.asset_id,
            inventory_item_id: row.inventory_item_id,
            asset_name: row.asset_name,
            asset_model: row.asset_model,
            laboratory_id: row.laboratory_id,
            laboratory_name: row.laboratory_name,
            schedule_name: row.schedule_name,
            interval_days: row.interval_days,
            next_maintenance_at: row.next_maintenance_at,
            remind_before_days: row.remind_before_days,
            public_notes: row.public_notes,
            internal_notes: show_sensitive.then_some(row.internal_notes).flatten(),
        }
    }
}
