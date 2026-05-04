use crate::authentication::Actor;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(crate) struct MaintenanceRecordRow {
    pub maintenance_record_id: Uuid,
    pub asset_id: Option<Uuid>,
    pub inventory_item_id: Option<Uuid>,
    pub asset_name: String,
    pub asset_model: Option<String>,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub maintenance_type: String,
    pub maintained_at: DateTime<Utc>,
    pub responsible_user_id: Option<Uuid>,
    pub responsible_username: Option<String>,
    pub description: String,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
    pub created_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub(crate) struct MaintenanceRecordResponse {
    pub maintenance_record_id: Uuid,
    pub asset_id: Option<Uuid>,
    pub inventory_item_id: Option<Uuid>,
    pub asset_name: String,
    pub asset_model: Option<String>,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub maintenance_type: String,
    pub maintained_at: DateTime<Utc>,
    pub responsible_user_id: Option<Uuid>,
    pub responsible_username: Option<String>,
    pub description: String,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
    pub created_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MaintenanceRecordResponse {
    pub fn from_row(row: MaintenanceRecordRow, actor: &Actor) -> Self {
        let show_sensitive = actor.is_owner() || actor.is_same_laboratory(row.laboratory_id);
        Self {
            maintenance_record_id: row.maintenance_record_id,
            asset_id: row.asset_id,
            inventory_item_id: row.inventory_item_id,
            asset_name: row.asset_name,
            asset_model: row.asset_model,
            laboratory_id: row.laboratory_id,
            laboratory_name: row.laboratory_name,
            maintenance_type: row.maintenance_type,
            maintained_at: row.maintained_at,
            responsible_user_id: row.responsible_user_id,
            responsible_username: row.responsible_username,
            description: row.description,
            public_notes: row.public_notes,
            internal_notes: show_sensitive.then_some(row.internal_notes).flatten(),
            created_by_user_id: row.created_by_user_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
