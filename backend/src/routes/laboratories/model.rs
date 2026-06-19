use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize)]
pub(super) struct LaboratoryResponse {
    laboratory_id: Uuid,
    name: String,
    address: String,
    description: Option<String>,
    contact: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub(super) struct LaboratoryRow {
    pub(super) laboratory_id: Uuid,
    pub(super) name: String,
    pub(super) address: String,
    pub(super) description: Option<String>,
    pub(super) contact: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

pub(super) fn create_laboratory_rollback_details(laboratory: &LaboratoryRow) -> Value {
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "laboratory",
            "where": {
                "laboratory_id": laboratory.laboratory_id,
            },
        },
    })
}

pub(super) fn update_laboratory_rollback_details(laboratory: &LaboratoryRow) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "laboratory",
            "where": {
                "laboratory_id": laboratory.laboratory_id,
            },
            "values": {
                "name": &laboratory.name,
                "address": &laboratory.address,
                "description": laboratory.description.as_deref(),
                "contact": laboratory.contact.as_deref(),
                "updated_at": &laboratory.updated_at,
            },
        },
    })
}

pub(super) fn delete_laboratory_rollback_details(laboratory: &LaboratoryRow) -> Value {
    json!({
        "rollback": {
            "operation": "create",
            "resource_type": "laboratory",
            "values": {
                "laboratory_id": laboratory.laboratory_id,
                "name": &laboratory.name,
                "address": &laboratory.address,
                "description": laboratory.description.as_deref(),
                "contact": laboratory.contact.as_deref(),
                "created_at": &laboratory.created_at,
                "updated_at": &laboratory.updated_at,
            },
        },
    })
}

impl From<LaboratoryRow> for LaboratoryResponse {
    fn from(row: LaboratoryRow) -> Self {
        Self {
            laboratory_id: row.laboratory_id,
            name: row.name,
            address: row.address,
            description: row.description,
            contact: row.contact,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(super) async fn fetch_laboratory(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Option<LaboratoryRow>, anyhow::Error> {
    sqlx::query_as!(
        LaboratoryRow,
        r#"
        SELECT laboratory_id, name, address, description, contact, created_at, updated_at
        FROM laboratories
        WHERE laboratory_id = $1
        "#,
        laboratory_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| anyhow!(e))
}
