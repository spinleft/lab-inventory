use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Serialize)]
pub(super) struct LocationResponse {
    location_id: Uuid,
    laboratory_id: Uuid,
    parent_location_id: Option<Uuid>,
    name: String,
    code: String,
    path: String,
    depth: i32,
    description: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct LocationRow {
    pub(super) location_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) parent_location_id: Option<Uuid>,
    pub(super) name: String,
    pub(super) code: String,
    pub(super) path: String,
    pub(super) depth: i32,
    pub(super) description: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

impl From<LocationRow> for LocationResponse {
    fn from(row: LocationRow) -> Self {
        Self {
            location_id: row.location_id,
            laboratory_id: row.laboratory_id,
            parent_location_id: row.parent_location_id,
            name: row.name,
            code: row.code,
            path: row.path,
            depth: row.depth,
            description: row.description,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(super) fn create_location_rollback_details(location: &LocationRow) -> Value {
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "location",
            "where": {
                "location_id": location.location_id,
            },
        },
    })
}

pub(super) fn update_location_rollback_details(location: &LocationRow) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "location",
            "where": {
                "location_id": location.location_id,
            },
            "values": {
                "laboratory_id": location.laboratory_id,
                "parent_location_id": location.parent_location_id,
                "name": &location.name,
                "code": &location.code,
                "path": &location.path,
                "depth": location.depth,
                "description": location.description.as_deref(),
                "updated_at": location.updated_at,
            },
        },
    })
}

pub(super) fn delete_location_rollback_details(
    locations: &[LocationRow],
    cleared_inventory_item_ids: &[Uuid],
) -> Value {
    json!({
        "rollback": {
            "operation": "restore_tree",
            "resource_type": "location",
            "values": {
                "locations": locations,
                "cleared_inventory_item_ids": cleared_inventory_item_ids,
            },
        },
    })
}
