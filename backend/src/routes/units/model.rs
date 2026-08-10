use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Serialize)]
pub(super) struct UnitResponse {
    unit_id: Uuid,
    laboratory_id: Uuid,
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
    pub(super) laboratory_id: Uuid,
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
            laboratory_id: row.laboratory_id,
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
                "laboratory_id": unit.laboratory_id,
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
