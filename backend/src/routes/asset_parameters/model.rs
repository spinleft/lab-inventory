use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Serialize)]
pub(super) struct AssetParameterResponse {
    parameter_type_id: Uuid,
    laboratory_id: Uuid,
    code: String,
    name: String,
    data_type: String,
    unit_dimension: Option<String>,
    default_unit_id: Option<Uuid>,
    description: Option<String>,
    options: Vec<AssetParameterOptionResponse>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetParameterRow {
    pub(super) parameter_type_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) code: String,
    pub(super) name: String,
    pub(super) data_type: String,
    pub(super) unit_dimension: Option<String>,
    pub(super) default_unit_id: Option<Uuid>,
    pub(super) description: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub(super) struct AssetParameterOptionResponse {
    option_id: Uuid,
    parameter_type_id: Uuid,
    code: String,
    label: String,
    sort_order: i32,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetParameterOptionRow {
    pub(super) option_id: Uuid,
    pub(super) parameter_type_id: Uuid,
    pub(super) code: String,
    pub(super) label: String,
    pub(super) sort_order: i32,
}

impl From<AssetParameterOptionRow> for AssetParameterOptionResponse {
    fn from(row: AssetParameterOptionRow) -> Self {
        Self {
            option_id: row.option_id,
            parameter_type_id: row.parameter_type_id,
            code: row.code,
            label: row.label,
            sort_order: row.sort_order,
        }
    }
}

impl AssetParameterResponse {
    pub(super) fn from_parts(
        row: AssetParameterRow,
        options: Vec<AssetParameterOptionRow>,
    ) -> Self {
        Self {
            parameter_type_id: row.parameter_type_id,
            laboratory_id: row.laboratory_id,
            code: row.code,
            name: row.name,
            data_type: row.data_type,
            unit_dimension: row.unit_dimension,
            default_unit_id: row.default_unit_id,
            description: row.description,
            options: options
                .into_iter()
                .map(|o| AssetParameterOptionResponse {
                    option_id: o.option_id,
                    parameter_type_id: o.parameter_type_id,
                    code: o.code,
                    label: o.label,
                    sort_order: o.sort_order,
                })
                .collect(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(super) fn create_asset_parameter_rollback_details(parameter: &AssetParameterRow) -> Value {
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "asset_parameter",
            "where": {
                "parameter_type_id": parameter.parameter_type_id,
            },
        },
    })
}

pub(super) fn update_asset_parameter_rollback_details(
    parameter: &AssetParameterRow,
    options: &[AssetParameterOptionRow],
) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "asset_parameter",
            "where": {
                "parameter_type_id": parameter.parameter_type_id,
            },
            "values": {
                "laboratory_id": parameter.laboratory_id,
                "code": &parameter.code,
                "name": &parameter.name,
                "data_type": &parameter.data_type,
                "unit_dimension": parameter.unit_dimension.as_deref(),
                "default_unit_id": parameter.default_unit_id,
                "description": parameter.description.as_deref(),
                "options": options,
                "updated_at": parameter.updated_at,
            },
        },
    })
}

pub(super) fn delete_asset_parameter_rollback_details(
    parameter: &AssetParameterRow,
    options: &[AssetParameterOptionRow],
) -> Value {
    json!({
        "rollback": {
            "operation": "create",
            "resource_type": "asset_parameter",
            "values": {
                "parameter": parameter,
                "options": options,
            },
        },
    })
}
