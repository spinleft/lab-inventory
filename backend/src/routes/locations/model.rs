use crate::domain::LaboratoryId;
use crate::domain::LocationId;
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
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

pub(super) async fn fetch_location(
    pool: &PgPool,
    location_id: LocationId,
) -> Result<Option<LocationRow>, anyhow::Error> {
    sqlx::query_as!(
        LocationRow,
        r#"
        SELECT
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        FROM locations
        WHERE location_id = $1
        "#,
        Uuid::from(location_id),
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch location")
}

pub(super) async fn fetch_location_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    location_id: LocationId,
) -> Result<Option<LocationRow>, anyhow::Error> {
    sqlx::query_as!(
        LocationRow,
        r#"
        SELECT
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        FROM locations
        WHERE location_id = $1
        FOR UPDATE
        "#,
        Uuid::from(location_id),
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch location for update")
}

pub(super) async fn fetch_location_tree_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<Vec<LocationRow>, anyhow::Error> {
    sqlx::query_as!(
        LocationRow,
        r#"
        SELECT
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        FROM locations
        WHERE laboratory_id = $1
          AND path <@ $2::text::ltree
        ORDER BY path
        FOR UPDATE
        "#,
        *laboratory_id,
        root_path,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch location tree for update")
}

pub(super) fn map_database_conflict(
    error: &sqlx::Error,
    duplicate_name: &str,
    duplicate_code: &str,
    duplicate_path: &str,
    generic_unique: &str,
) -> Option<String> {
    let sqlx::Error::Database(database_error) = error else {
        return None;
    };

    match (
        database_error.code().as_deref(),
        database_error.constraint(),
    ) {
        (Some("23505"), Some("uq_locations_sibling_name")) => Some(duplicate_name.into()),
        (Some("23505"), Some("uq_locations_sibling_code")) => Some(duplicate_code.into()),
        (Some("23505"), Some("uq_locations_path")) => Some(duplicate_path.into()),
        (Some("23505"), _) => Some(generic_unique.into()),
        _ => None,
    }
}
