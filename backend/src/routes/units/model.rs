use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Serialize)]
pub(super) struct UnitResponse {
    unit_id: Uuid,
    code: String,
    name: String,
    symbol: String,
    dimension: String,
    scale_to_base: f64,
    allow_decimal: bool,
    created_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct UnitRow {
    pub(super) unit_id: Uuid,
    pub(super) code: String,
    pub(super) name: String,
    pub(super) symbol: String,
    pub(super) dimension: String,
    pub(super) scale_to_base: f64,
    pub(super) allow_decimal: bool,
    pub(super) created_at: DateTime<Utc>,
}

impl From<UnitRow> for UnitResponse {
    fn from(row: UnitRow) -> Self {
        Self {
            unit_id: row.unit_id,
            code: row.code,
            name: row.name,
            symbol: row.symbol,
            dimension: row.dimension,
            scale_to_base: row.scale_to_base,
            allow_decimal: row.allow_decimal,
            created_at: row.created_at,
        }
    }
}

pub(super) fn create_unit_rollback_details(unit: &UnitRow) -> Value {
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "unit",
            "where": {
                "unit_id": unit.unit_id,
            },
        },
    })
}

pub(super) fn update_unit_rollback_details(unit: &UnitRow) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "unit",
            "where": {
                "unit_id": unit.unit_id,
            },
            "values": {
                "code": &unit.code,
                "name": &unit.name,
                "symbol": &unit.symbol,
                "dimension": &unit.dimension,
                "scale_to_base": unit.scale_to_base,
                "allow_decimal": unit.allow_decimal,
            },
        },
    })
}

pub(super) fn delete_unit_rollback_details(unit: &UnitRow) -> Value {
    json!({
        "rollback": {
            "operation": "create",
            "resource_type": "unit",
            "values": {
                "unit_id": unit.unit_id,
                "code": &unit.code,
                "name": &unit.name,
                "symbol": &unit.symbol,
                "dimension": &unit.dimension,
                "scale_to_base": unit.scale_to_base,
                "allow_decimal": unit.allow_decimal,
                "created_at": unit.created_at,
            },
        },
    })
}

pub(super) async fn fetch_unit(
    pool: &PgPool,
    unit_id: Uuid,
) -> Result<Option<UnitRow>, anyhow::Error> {
    sqlx::query_as!(
        UnitRow,
        r#"
        SELECT unit_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        FROM units
        WHERE unit_id = $1
        "#,
        unit_id,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch unit")
}

pub(super) async fn fetch_unit_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> Result<Option<UnitRow>, anyhow::Error> {
    sqlx::query_as!(
        UnitRow,
        r#"
        SELECT unit_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        FROM units
        WHERE unit_id = $1
        FOR UPDATE
        "#,
        unit_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch unit for update")
}

pub(super) enum UnitDatabaseError {
    Conflict(String),
    Validation(String),
}

pub(super) fn map_unit_database_error(
    error: &sqlx::Error,
    duplicate_code: &str,
    generic_unique: &str,
    invalid_unit: &str,
    invalid_dimension: &str,
) -> Option<UnitDatabaseError> {
    let sqlx::Error::Database(database_error) = error else {
        return None;
    };

    match (
        database_error.code().as_deref(),
        database_error.constraint(),
    ) {
        (Some("23505"), Some("units_code_key")) => {
            Some(UnitDatabaseError::Conflict(duplicate_code.into()))
        }
        (Some("23505"), _) => Some(UnitDatabaseError::Conflict(generic_unique.into())),
        (Some("23514"), _) => Some(UnitDatabaseError::Validation(invalid_unit.into())),
        (Some("23503"), Some("units_dimension_fkey")) => {
            Some(UnitDatabaseError::Validation(invalid_dimension.into()))
        }
        (Some("23503"), _) => Some(UnitDatabaseError::Validation(
            "Invalid referenced record".into(),
        )),
        _ => None,
    }
}
