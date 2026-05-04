use crate::authentication::Actor;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(crate) struct MaintenanceScheduleRow {
    pub maintenance_schedule_id: Uuid,
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
    pub is_active: bool,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
    pub created_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub(crate) struct MaintenanceScheduleResponse {
    pub maintenance_schedule_id: Uuid,
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
    pub is_active: bool,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
    pub created_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MaintenanceScheduleResponse {
    pub fn from_row(row: MaintenanceScheduleRow, actor: &Actor) -> Self {
        let show_sensitive = actor.is_owner() || actor.is_same_laboratory(row.laboratory_id);
        Self {
            maintenance_schedule_id: row.maintenance_schedule_id,
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
            is_active: row.is_active,
            public_notes: row.public_notes,
            internal_notes: show_sensitive.then_some(row.internal_notes).flatten(),
            created_by_user_id: row.created_by_user_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
