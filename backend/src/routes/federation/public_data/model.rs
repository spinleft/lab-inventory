use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) enum FederationReadTarget {
    Laboratory,
    Assets,
    Asset(Uuid),
    AssetAttachments(Uuid),
    AssetCategories,
    AssetCategory(Uuid),
    AssetParameters,
    AssetParameter(Uuid),
    InventoryItems,
    InventoryItem(Uuid),
    InventoryItemAttachments(Uuid),
    Locations,
    Location(Uuid),
    Attachments,
    Attachment(Uuid),
    AttachmentDownload(Uuid),
}

#[derive(Serialize)]
pub(super) struct PaginatedJson<T> {
    pub(super) items: Vec<T>,
    pub(super) limit: i64,
    pub(super) offset: i64,
    pub(super) total: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct LaboratoryPublicRow {
    pub(super) laboratory_id: Uuid,
    pub(super) name: String,
    pub(super) address: String,
    pub(super) description: Option<String>,
    pub(super) contact: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct AssetPublicRow {
    pub(super) asset_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) category_id: Option<Uuid>,
    pub(super) tracking_mode: String,
    pub(super) name: String,
    pub(super) model: Option<String>,
    pub(super) manufacturer: Option<String>,
    pub(super) inventory_unit_id: Uuid,
    pub(super) public_notes: Option<String>,
    pub(super) inventory_item_count: i64,
    pub(super) quantity_on_hand: f64,
    pub(super) quantity_allocated: f64,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub(super) struct AssetPublicResponse {
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
    pub(super) inventory_summary: AssetInventorySummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) inventory_items: Option<Vec<InventoryItemPublicResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) parameters: Option<Vec<ParameterValueResponse>>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub(super) struct AssetInventorySummary {
    pub(super) item_count: i64,
    pub(super) quantity_on_hand: f64,
    pub(super) quantity_allocated: f64,
}

impl AssetPublicResponse {
    pub(super) fn from_row(
        row: AssetPublicRow,
        inventory_items: Option<Vec<InventoryItemPublicResponse>>,
        parameters: Option<Vec<ParameterValueResponse>>,
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
            internal_notes: None,
            inventory_summary: AssetInventorySummary {
                item_count: row.inventory_item_count,
                quantity_on_hand: row.quantity_on_hand,
                quantity_allocated: row.quantity_allocated,
            },
            inventory_items,
            parameters,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct InventoryItemPublicRow {
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
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) last_stocktake_at: Option<DateTime<Utc>>,
    pub(super) asset_category_id: Option<Uuid>,
    pub(super) asset_name: String,
    pub(super) asset_model: Option<String>,
    pub(super) asset_manufacturer: Option<String>,
    pub(super) asset_inventory_unit_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct InventoryItemPublicResponse {
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
    pub(super) asset: InventoryItemAssetResponse,
}

#[derive(Serialize)]
pub(super) struct InventoryItemAssetResponse {
    pub(super) asset_id: Uuid,
    pub(super) category_id: Option<Uuid>,
    pub(super) name: String,
    pub(super) model: Option<String>,
    pub(super) manufacturer: Option<String>,
    pub(super) inventory_unit_id: Uuid,
}

impl From<InventoryItemPublicRow> for InventoryItemPublicResponse {
    fn from(row: InventoryItemPublicRow) -> Self {
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
            internal_notes: None,
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_stocktake_at: row.last_stocktake_at,
            asset: InventoryItemAssetResponse {
                asset_id: row.asset_id,
                category_id: row.asset_category_id,
                name: row.asset_name,
                model: row.asset_model,
                manufacturer: row.asset_manufacturer,
                inventory_unit_id: row.asset_inventory_unit_id,
            },
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct CategoryRow {
    pub(super) category_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) parent_category_id: Option<Uuid>,
    pub(super) name: String,
    pub(super) code: String,
    pub(super) path: String,
    pub(super) depth: i32,
    pub(super) description: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
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

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct ParameterRow {
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

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct ParameterOptionRow {
    pub(super) option_id: Uuid,
    pub(super) parameter_type_id: Uuid,
    pub(super) code: String,
    pub(super) label: String,
    pub(super) sort_order: i32,
}

#[derive(Serialize)]
pub(super) struct ParameterResponse {
    pub(super) parameter_type_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) code: String,
    pub(super) name: String,
    pub(super) data_type: String,
    pub(super) unit_dimension: Option<String>,
    pub(super) default_unit_id: Option<Uuid>,
    pub(super) description: Option<String>,
    pub(super) options: Vec<ParameterOptionRow>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub(super) struct ParameterValueRow {
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

#[derive(Serialize)]
pub(super) struct ParameterValueResponse {
    pub(super) value_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) asset_id: Uuid,
    pub(super) parameter_type_id: Uuid,
    pub(super) code: String,
    pub(super) name: String,
    pub(super) data_type: String,
    pub(super) unit_dimension: Option<String>,
    pub(super) default_unit_id: Option<Uuid>,
    pub(super) value: Value,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

impl From<ParameterValueRow> for ParameterValueResponse {
    fn from(row: ParameterValueRow) -> Self {
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
            laboratory_id: row.laboratory_id,
            asset_id: row.asset_id,
            parameter_type_id: row.parameter_type_id,
            code: row.code,
            name: row.name,
            data_type: row.data_type,
            unit_dimension: row.unit_dimension,
            default_unit_id: row.default_unit_id,
            value,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct AttachmentPublicRow {
    pub(super) attachment_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) asset_id: Option<Uuid>,
    pub(super) inventory_item_id: Option<Uuid>,
    pub(super) display_name: String,
    pub(super) original_file_name: String,
    pub(super) description: Option<String>,
    pub(super) mime_type: Option<String>,
    pub(super) file_size_bytes: i64,
    sha256_hex: String,
    pub(super) is_public: bool,
    pub(super) uploaded_by_user_id: Option<Uuid>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub(super) struct AttachmentDownloadRow {
    pub(super) storage_key: String,
    pub(super) original_file_name: String,
    pub(super) mime_type: Option<String>,
}
