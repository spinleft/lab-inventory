use chrono::{DateTime, NaiveDate, Utc};
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

/// One stored parameter value, with everything a delete or a data type change
/// has to know about it: which asset holds it, the column layout for the type it
/// currently has, and — for enum values — the option it points at.
#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetParameterValueRow {
    pub(super) value_id: Uuid,
    pub(super) asset_id: Uuid,
    pub(super) asset_name: String,
    pub(super) asset_model: Option<String>,
    pub(super) parameter_type_id: Uuid,
    pub(super) data_type: String,
    pub(super) value_text: Option<String>,
    pub(super) value_number: Option<f64>,
    pub(super) value_number_in_base: Option<f64>,
    pub(super) value_range_start: Option<f64>,
    pub(super) value_range_end: Option<f64>,
    pub(super) value_range_start_in_base: Option<f64>,
    pub(super) value_range_end_in_base: Option<f64>,
    pub(super) unit_id: Option<Uuid>,
    pub(super) value_boolean: Option<bool>,
    pub(super) value_date: Option<NaiveDate>,
    pub(super) value_option_id: Option<Uuid>,
    pub(super) option_code: Option<String>,
    pub(super) option_label: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetParameterAssignmentRow {
    pub(super) assignment_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) parameter_type_id: Uuid,
    pub(super) category_id: Option<Uuid>,
    pub(super) asset_id: Option<Uuid>,
    pub(super) default_unit_id: Option<Uuid>,
    pub(super) applies_to_descendants: bool,
    pub(super) is_required: bool,
    pub(super) sort_order: i32,
    pub(super) created_at: DateTime<Utc>,
}

/// A stored value as the data type change screen needs it: which asset holds it
/// and what it currently reads as.
#[derive(Serialize)]
pub(super) struct AssetParameterValueResponse {
    value_id: Uuid,
    asset_id: Uuid,
    asset_name: String,
    asset_model: Option<String>,
    data_type: String,
    value: Value,
}

impl From<AssetParameterValueRow> for AssetParameterValueResponse {
    fn from(row: AssetParameterValueRow) -> Self {
        let value = match row.data_type.as_str() {
            "text" => json!({ "text": row.value_text }),
            "number" => json!({
                "number": row.value_number,
                "number_in_base": row.value_number_in_base,
                "unit_id": row.unit_id,
            }),
            "range" => json!({
                "range_start": row.value_range_start,
                "range_end": row.value_range_end,
                "range_start_in_base": row.value_range_start_in_base,
                "range_end_in_base": row.value_range_end_in_base,
                "unit_id": row.unit_id,
            }),
            "boolean" => json!({ "boolean": row.value_boolean }),
            "date" => json!({ "date": row.value_date.map(|date| date.to_string()) }),
            "enum" => json!({
                "option_id": row.value_option_id,
                "option_code": row.option_code,
                "option_label": row.option_label,
            }),
            _ => Value::Null,
        };
        Self {
            value_id: row.value_id,
            asset_id: row.asset_id,
            asset_name: row.asset_name,
            asset_model: row.asset_model,
            data_type: row.data_type,
            value,
        }
    }
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
    parameter_values: &[AssetParameterValueRow],
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
                "parameter_values": parameter_values,
                "updated_at": parameter.updated_at,
            },
        },
    })
}

pub(super) fn delete_asset_parameter_rollback_details(
    parameter: &AssetParameterRow,
    options: &[AssetParameterOptionRow],
    assignments: &[AssetParameterAssignmentRow],
    parameter_values: &[AssetParameterValueRow],
) -> Value {
    json!({
        "rollback": {
            "operation": "create",
            "resource_type": "asset_parameter",
            "values": {
                "parameter": parameter,
                "options": options,
                "assignments": assignments,
                "parameter_values": parameter_values,
            },
        },
    })
}
