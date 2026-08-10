use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(super) struct DeletedAttachmentRow {
    pub(super) attachment_id: Uuid,
    pub(super) storage_key: String,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetRow {
    pub(super) asset_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) category_id: Option<Uuid>,
    pub(super) tracking_mode: String,
    pub(super) name: String,
    pub(super) model: Option<String>,
    pub(super) manufacturer: Option<String>,
    pub(super) inventory_unit_id: Uuid,
    pub(super) public_notes: Option<String>,
    pub(super) internal_notes: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) inventory_item_count: i64,
    pub(super) quantity_on_hand: f64,
    pub(super) quantity_allocated: f64,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetInventoryItemRow {
    pub(super) inventory_item_id: Uuid,
    pub(super) asset_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) tracking_mode: String,
    pub(super) serial_number: Option<String>,
    pub(super) batch_number: Option<String>,
    pub(super) quantity_on_hand: f64,
    pub(super) quantity_allocated: f64,
    pub(super) location_id: Option<Uuid>,
    pub(super) status: String,
    pub(super) public_notes: Option<String>,
    pub(super) internal_notes: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) last_stocktake_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetParameterValueRow {
    pub(super) value_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) asset_id: Uuid,
    pub(super) parameter_type_id: Uuid,
    pub(super) code: String,
    pub(super) name: String,
    pub(super) data_type: String,
    pub(super) unit_dimension: Option<String>,
    pub(super) default_unit_id: Option<Uuid>,
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

#[derive(Clone, sqlx::FromRow)]
pub(super) struct AssetParameterDefinitionRow {
    pub(super) parameter_type_id: Uuid,
    pub(super) data_type: String,
    pub(super) unit_dimension: Option<String>,
    pub(super) default_unit_id: Option<Uuid>,
}

#[derive(Clone, sqlx::FromRow)]
pub(super) struct UnitRow {
    pub(super) unit_id: Uuid,
    pub(super) dimension: String,
    pub(super) scale_to_base: f64,
}

#[derive(Serialize)]
pub(super) struct AssetInventorySummary {
    item_count: i64,
    quantity_on_hand: f64,
    quantity_allocated: f64,
}

#[derive(Serialize)]
pub(super) struct AssetResponse {
    asset_id: Uuid,
    laboratory_id: Uuid,
    category_id: Option<Uuid>,
    tracking_mode: String,
    name: String,
    model: Option<String>,
    manufacturer: Option<String>,
    inventory_unit_id: Uuid,
    public_notes: Option<String>,
    internal_notes: Option<String>,
    inventory_summary: AssetInventorySummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    inventory_items: Option<Vec<AssetInventoryItemResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Vec<AssetParameterValueResponse>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub(super) struct AssetInventoryItemResponse {
    inventory_item_id: Uuid,
    asset_id: Uuid,
    laboratory_id: Uuid,
    tracking_mode: String,
    serial_number: Option<String>,
    batch_number: Option<String>,
    quantity_on_hand: f64,
    quantity_allocated: f64,
    location_id: Option<Uuid>,
    status: String,
    public_notes: Option<String>,
    internal_notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_stocktake_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub(super) struct AssetParameterValueResponse {
    value_id: Uuid,
    laboratory_id: Uuid,
    asset_id: Uuid,
    parameter_type_id: Uuid,
    code: String,
    name: String,
    data_type: String,
    unit_dimension: Option<String>,
    default_unit_id: Option<Uuid>,
    value: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub(super) struct AssetParameterValueInput {
    pub(super) parameter_type_id: Uuid,
    pub(super) value: Option<Value>,
}

/// A parameter value that has been validated and expanded into the column layout
/// of `asset_parameter_values`, ready to be written by the query layer.
pub(super) struct ResolvedAssetParameterValue {
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
}

impl AssetResponse {
    pub(super) fn from_parts(
        row: AssetRow,
        inventory_items: Option<Vec<AssetInventoryItemRow>>,
        parameters: Option<Vec<AssetParameterValueRow>>,
        include_internal_notes: bool,
    ) -> Self {
        Self {
            asset_id: row.asset_id,
            laboratory_id: row.laboratory_id,
            category_id: row.category_id,
            tracking_mode: row.tracking_mode,
            name: row.name,
            model: row.model,
            manufacturer: row.manufacturer,
            inventory_unit_id: row.inventory_unit_id,
            public_notes: row.public_notes,
            internal_notes: if include_internal_notes {
                row.internal_notes
            } else {
                None
            },
            inventory_summary: AssetInventorySummary {
                item_count: row.inventory_item_count,
                quantity_on_hand: row.quantity_on_hand,
                quantity_allocated: row.quantity_allocated,
            },
            inventory_items: inventory_items.map(|items| {
                items
                    .into_iter()
                    .map(|item| AssetInventoryItemResponse::from_row(item, include_internal_notes))
                    .collect()
            }),
            parameters: parameters.map(|parameters| {
                parameters
                    .into_iter()
                    .map(AssetParameterValueResponse::from)
                    .collect()
            }),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl AssetInventoryItemResponse {
    fn from_row(row: AssetInventoryItemRow, include_internal_notes: bool) -> Self {
        Self {
            inventory_item_id: row.inventory_item_id,
            asset_id: row.asset_id,
            laboratory_id: row.laboratory_id,
            tracking_mode: row.tracking_mode,
            serial_number: row.serial_number,
            batch_number: row.batch_number,
            quantity_on_hand: row.quantity_on_hand,
            quantity_allocated: row.quantity_allocated,
            location_id: row.location_id,
            status: row.status,
            public_notes: row.public_notes,
            internal_notes: if include_internal_notes {
                row.internal_notes
            } else {
                None
            },
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_stocktake_at: row.last_stocktake_at,
        }
    }
}

impl From<AssetParameterValueRow> for AssetParameterValueResponse {
    fn from(row: AssetParameterValueRow) -> Self {
        let value = parameter_value_json(&row);
        Self {
            value_id: row.value_id,
            laboratory_id: row.laboratory_id,
            asset_id: row.asset_id,
            parameter_type_id: row.parameter_type_id,
            code: row.code,
            name: row.name,
            data_type: row.data_type.clone(),
            unit_dimension: row.unit_dimension,
            default_unit_id: row.default_unit_id,
            value,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(super) fn create_asset_rollback_details(asset: &AssetRow) -> Value {
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "asset",
            "where": {
                "asset_id": asset.asset_id,
            },
        },
    })
}

pub(super) fn update_asset_rollback_details(
    asset: &AssetRow,
    parameter_values: &[AssetParameterValueRow],
) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "asset",
            "where": {
                "asset_id": asset.asset_id,
            },
            "values": {
                "laboratory_id": asset.laboratory_id,
                "category_id": asset.category_id,
                "tracking_mode": &asset.tracking_mode,
                "name": &asset.name,
                "model": asset.model.as_deref(),
                "manufacturer": asset.manufacturer.as_deref(),
                "inventory_unit_id": asset.inventory_unit_id,
                "public_notes": asset.public_notes.as_deref(),
                "internal_notes": asset.internal_notes.as_deref(),
                "parameter_values": parameter_values,
                "updated_at": asset.updated_at,
            },
        },
    })
}

pub(super) fn delete_asset_rollback_details(
    asset: &AssetRow,
    inventory_items: &[AssetInventoryItemRow],
    parameter_values: &[AssetParameterValueRow],
    attachment_ids: &[Uuid],
) -> Value {
    json!({
        "rollback": {
            "operation": "create",
            "resource_type": "asset",
            "values": {
                "asset": asset,
                "inventory_items": inventory_items,
                "parameter_values": parameter_values,
                "deleted_attachment_ids": attachment_ids,
            },
        },
    })
}

pub(super) fn parse_include(include: Option<&str>) -> Result<bool, String> {
    let Some(include) = include else {
        return Ok(false);
    };
    let includes: Vec<_> = include
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    for include in &includes {
        if *include != "parameters" {
            return Err(format!("Unsupported include: {include}"));
        }
    }

    Ok(includes.contains(&"parameters"))
}

fn parameter_value_json(row: &AssetParameterValueRow) -> Value {
    match row.data_type.as_str() {
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
    }
}
