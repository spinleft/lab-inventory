use crate::domain::{
    AssetParameterDataType, AssetParameterValue, AssetTrackingMode, LaboratoryId, NewInventoryItem,
};
use crate::utils::error_chain_fmt;
use anyhow::Context;
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
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
    pub(super) default_unit_id: Uuid,
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
    pub(super) quantity_unit_id: Uuid,
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
    pub(super) value_number_base: Option<f64>,
    pub(super) value_range_start: Option<f64>,
    pub(super) value_range_end: Option<f64>,
    pub(super) value_range_start_base: Option<f64>,
    pub(super) value_range_end_base: Option<f64>,
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
struct AssetParameterDefinitionRow {
    parameter_type_id: Uuid,
    data_type: String,
    unit_dimension: Option<String>,
    default_unit_id: Option<Uuid>,
}

#[derive(Clone, sqlx::FromRow)]
struct UnitRow {
    unit_id: Uuid,
    dimension: String,
    scale_to_base: f64,
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
    default_unit_id: Uuid,
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
    quantity_unit_id: Uuid,
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

struct ResolvedAssetParameterValue {
    parameter_type_id: Uuid,
    data_type: String,
    value_text: Option<String>,
    value_number: Option<f64>,
    value_number_base: Option<f64>,
    value_range_start: Option<f64>,
    value_range_end: Option<f64>,
    value_range_start_base: Option<f64>,
    value_range_end_base: Option<f64>,
    unit_id: Option<Uuid>,
    value_boolean: Option<bool>,
    value_date: Option<NaiveDate>,
    value_option_id: Option<Uuid>,
}

#[derive(thiserror::Error)]
pub(super) enum AssetDatabaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for AssetDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
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
            default_unit_id: row.default_unit_id,
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
            quantity_unit_id: row.quantity_unit_id,
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
                "default_unit_id": asset.default_unit_id,
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

pub(super) fn asset_select() -> &'static str {
    r#"
    SELECT
        assets.asset_id,
        assets.laboratory_id,
        assets.category_id,
        assets.tracking_mode,
        assets.name,
        assets.model,
        assets.manufacturer,
        assets.default_unit_id,
        assets.public_notes,
        assets.internal_notes,
        assets.created_at,
        assets.updated_at,
        (
            SELECT COUNT(*)
            FROM asset_inventory_items AS inventory_items
            WHERE inventory_items.asset_id = assets.asset_id
        ) AS inventory_item_count,
        (
            SELECT COALESCE(SUM(inventory_items.quantity_on_hand), 0)::double precision
            FROM asset_inventory_items AS inventory_items
            WHERE inventory_items.asset_id = assets.asset_id
        ) AS quantity_on_hand,
        (
            SELECT COALESCE(SUM(inventory_items.quantity_allocated), 0)::double precision
            FROM asset_inventory_items AS inventory_items
            WHERE inventory_items.asset_id = assets.asset_id
        ) AS quantity_allocated
    FROM assets
    "#
}

pub(super) async fn fetch_asset(
    pool: &PgPool,
    asset_id: Uuid,
) -> Result<Option<AssetRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetRow,
        r#"
        SELECT
            assets.asset_id,
            assets.laboratory_id,
            assets.category_id,
            assets.tracking_mode,
            assets.name,
            assets.model,
            assets.manufacturer,
            assets.default_unit_id,
            assets.public_notes,
            assets.internal_notes,
            assets.created_at,
            assets.updated_at,
            (
                SELECT COUNT(*)
                FROM asset_inventory_items AS inventory_items
                WHERE inventory_items.asset_id = assets.asset_id
            ) AS "inventory_item_count!",
            (
                SELECT COALESCE(SUM(inventory_items.quantity_on_hand), 0)::double precision
                FROM asset_inventory_items AS inventory_items
                WHERE inventory_items.asset_id = assets.asset_id
            ) AS "quantity_on_hand!",
            (
                SELECT COALESCE(SUM(inventory_items.quantity_allocated), 0)::double precision
                FROM asset_inventory_items AS inventory_items
                WHERE inventory_items.asset_id = assets.asset_id
            ) AS "quantity_allocated!"
        FROM assets
        WHERE assets.asset_id = $1
        "#,
        asset_id,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch asset")
}

pub(super) async fn fetch_asset_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
) -> Result<Option<AssetRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetRow,
        r#"
        SELECT
            assets.asset_id,
            assets.laboratory_id,
            assets.category_id,
            assets.tracking_mode,
            assets.name,
            assets.model,
            assets.manufacturer,
            assets.default_unit_id,
            assets.public_notes,
            assets.internal_notes,
            assets.created_at,
            assets.updated_at,
            (
                SELECT COUNT(*)
                FROM asset_inventory_items AS inventory_items
                WHERE inventory_items.asset_id = assets.asset_id
            ) AS "inventory_item_count!",
            (
                SELECT COALESCE(SUM(inventory_items.quantity_on_hand), 0)::double precision
                FROM asset_inventory_items AS inventory_items
                WHERE inventory_items.asset_id = assets.asset_id
            ) AS "quantity_on_hand!",
            (
                SELECT COALESCE(SUM(inventory_items.quantity_allocated), 0)::double precision
                FROM asset_inventory_items AS inventory_items
                WHERE inventory_items.asset_id = assets.asset_id
            ) AS "quantity_allocated!"
        FROM assets
        WHERE assets.asset_id = $1
        FOR UPDATE
        "#,
        asset_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch asset for update")
}

pub(super) async fn fetch_inventory_items_for_asset(
    pool: &PgPool,
    asset_id: Uuid,
) -> Result<Vec<AssetInventoryItemRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetInventoryItemRow,
        r#"
        SELECT
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            serial_number,
            batch_number,
            quantity_on_hand::double precision AS "quantity_on_hand!",
            quantity_allocated::double precision AS "quantity_allocated!",
            quantity_unit_id,
            location_id,
            status,
            public_notes,
            internal_notes,
            created_at,
            updated_at,
            last_stocktake_at
        FROM asset_inventory_items
        WHERE asset_id = $1
        ORDER BY created_at, inventory_item_id
        "#,
        asset_id,
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch inventory items")
}

pub(super) async fn fetch_inventory_items_for_asset_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
) -> Result<Vec<AssetInventoryItemRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetInventoryItemRow,
        r#"
        SELECT
            inventory_item_id,
            asset_id,
            laboratory_id,
            tracking_mode,
            serial_number,
            batch_number,
            quantity_on_hand::double precision AS "quantity_on_hand!",
            quantity_allocated::double precision AS "quantity_allocated!",
            quantity_unit_id,
            location_id,
            status,
            public_notes,
            internal_notes,
            created_at,
            updated_at,
            last_stocktake_at
        FROM asset_inventory_items
        WHERE asset_id = $1
        ORDER BY created_at, inventory_item_id
        FOR UPDATE
        "#,
        asset_id,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch inventory items for update")
}

pub(super) async fn fetch_parameter_values_for_asset(
    pool: &PgPool,
    asset_id: Uuid,
) -> Result<Vec<AssetParameterValueRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetParameterValueRow,
        r#"
        SELECT
            asset_parameter_values.value_id,
            asset_parameter_values.laboratory_id,
            asset_parameter_values.asset_id,
            asset_parameter_values.parameter_type_id,
            asset_parameter_types.code,
            asset_parameter_types.name,
            asset_parameter_values.data_type::text AS "data_type!",
            asset_parameter_types.unit_dimension,
            asset_parameter_types.default_unit_id,
            asset_parameter_values.value_text,
            asset_parameter_values.value_number,
            asset_parameter_values.value_number_base,
            asset_parameter_values.value_range_start,
            asset_parameter_values.value_range_end,
            asset_parameter_values.value_range_start_base,
            asset_parameter_values.value_range_end_base,
            asset_parameter_values.unit_id,
            asset_parameter_values.value_boolean,
            asset_parameter_values.value_date,
            asset_parameter_values.value_option_id,
            asset_parameter_options.code AS "option_code?",
            asset_parameter_options.label AS "option_label?",
            asset_parameter_values.created_at,
            asset_parameter_values.updated_at
        FROM asset_parameter_values
        JOIN asset_parameter_types
          ON asset_parameter_types.parameter_type_id = asset_parameter_values.parameter_type_id
        LEFT JOIN asset_parameter_options
          ON asset_parameter_options.option_id = asset_parameter_values.value_option_id
        WHERE asset_parameter_values.asset_id = $1
        ORDER BY asset_parameter_types.name, asset_parameter_types.code
        "#,
        asset_id,
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch asset parameter values")
}

pub(super) async fn fetch_parameter_values_for_assets(
    pool: &PgPool,
    asset_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<AssetParameterValueRow>>, anyhow::Error> {
    if asset_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as!(
        AssetParameterValueRow,
        r#"
        SELECT
            asset_parameter_values.value_id,
            asset_parameter_values.laboratory_id,
            asset_parameter_values.asset_id,
            asset_parameter_values.parameter_type_id,
            asset_parameter_types.code,
            asset_parameter_types.name,
            asset_parameter_values.data_type::text AS "data_type!",
            asset_parameter_types.unit_dimension,
            asset_parameter_types.default_unit_id,
            asset_parameter_values.value_text,
            asset_parameter_values.value_number,
            asset_parameter_values.value_number_base,
            asset_parameter_values.value_range_start,
            asset_parameter_values.value_range_end,
            asset_parameter_values.value_range_start_base,
            asset_parameter_values.value_range_end_base,
            asset_parameter_values.unit_id,
            asset_parameter_values.value_boolean,
            asset_parameter_values.value_date,
            asset_parameter_values.value_option_id,
            asset_parameter_options.code AS "option_code?",
            asset_parameter_options.label AS "option_label?",
            asset_parameter_values.created_at,
            asset_parameter_values.updated_at
        FROM asset_parameter_values
        JOIN asset_parameter_types
          ON asset_parameter_types.parameter_type_id = asset_parameter_values.parameter_type_id
        LEFT JOIN asset_parameter_options
          ON asset_parameter_options.option_id = asset_parameter_values.value_option_id
        WHERE asset_parameter_values.asset_id = ANY($1)
        ORDER BY
            asset_parameter_values.asset_id,
            asset_parameter_types.name,
            asset_parameter_types.code
        "#,
        asset_ids,
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch asset parameter values")?;

    let mut values_by_asset_id = HashMap::new();
    for row in rows {
        values_by_asset_id
            .entry(row.asset_id)
            .or_insert_with(Vec::new)
            .push(row);
    }

    Ok(values_by_asset_id)
}

pub(super) async fn fetch_parameter_values_for_asset_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
) -> Result<Vec<AssetParameterValueRow>, anyhow::Error> {
    sqlx::query_as!(
        AssetParameterValueRow,
        r#"
        SELECT
            asset_parameter_values.value_id,
            asset_parameter_values.laboratory_id,
            asset_parameter_values.asset_id,
            asset_parameter_values.parameter_type_id,
            asset_parameter_types.code,
            asset_parameter_types.name,
            asset_parameter_values.data_type::text AS "data_type!",
            asset_parameter_types.unit_dimension,
            asset_parameter_types.default_unit_id,
            asset_parameter_values.value_text,
            asset_parameter_values.value_number,
            asset_parameter_values.value_number_base,
            asset_parameter_values.value_range_start,
            asset_parameter_values.value_range_end,
            asset_parameter_values.value_range_start_base,
            asset_parameter_values.value_range_end_base,
            asset_parameter_values.unit_id,
            asset_parameter_values.value_boolean,
            asset_parameter_values.value_date,
            asset_parameter_values.value_option_id,
            asset_parameter_options.code AS "option_code?",
            asset_parameter_options.label AS "option_label?",
            asset_parameter_values.created_at,
            asset_parameter_values.updated_at
        FROM asset_parameter_values
        JOIN asset_parameter_types
          ON asset_parameter_types.parameter_type_id = asset_parameter_values.parameter_type_id
        LEFT JOIN asset_parameter_options
          ON asset_parameter_options.option_id = asset_parameter_values.value_option_id
        WHERE asset_parameter_values.asset_id = $1
        ORDER BY asset_parameter_types.name, asset_parameter_types.code
        FOR UPDATE OF asset_parameter_values
        "#,
        asset_id,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter values for update")
}

pub(super) async fn validate_category(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    category_id: Option<Uuid>,
) -> Result<(), AssetDatabaseError> {
    let Some(category_id) = category_id else {
        return Ok(());
    };

    let found = sqlx::query_scalar!(
        r#"
        SELECT category_id
        FROM asset_categories
        WHERE laboratory_id = $1
          AND category_id = $2
        FOR UPDATE
        "#,
        *laboratory_id,
        category_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch asset category")?;

    if found.is_some() {
        Ok(())
    } else {
        Err(AssetDatabaseError::Validation(
            "Asset category does not belong to this laboratory".into(),
        ))
    }
}

pub(super) async fn insert_inventory_items(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
    tracking_mode: AssetTrackingMode,
    items: &[NewInventoryItem],
) -> Result<Vec<AssetInventoryItemRow>, AssetDatabaseError> {
    let mut rows = Vec::with_capacity(items.len());
    for item in items {
        if let Some(location_id) = item.location_id {
            validate_location(transaction, laboratory_id, location_id.into()).await?;
        }

        let row = sqlx::query_as!(
            AssetInventoryItemRow,
            r#"
            INSERT INTO asset_inventory_items (
                inventory_item_id,
                asset_id,
                laboratory_id,
                tracking_mode,
                serial_number,
                batch_number,
                quantity_on_hand,
                quantity_allocated,
                quantity_unit_id,
                location_id,
                status,
                public_notes,
                internal_notes
            )
            VALUES (
                $1, $2, $3, $4, $5, $6,
                $7::double precision::numeric,
                $8::double precision::numeric,
                $9, $10, $11, $12, $13
            )
            RETURNING
                inventory_item_id,
                asset_id,
                laboratory_id,
                tracking_mode,
                serial_number,
                batch_number,
                quantity_on_hand::double precision AS "quantity_on_hand!",
                quantity_allocated::double precision AS "quantity_allocated!",
                quantity_unit_id,
                location_id,
                status,
                public_notes,
                internal_notes,
                created_at,
                updated_at,
                last_stocktake_at
            "#,
            Uuid::new_v4(),
            asset_id,
            *laboratory_id,
            tracking_mode.as_str(),
            item.serial_number.as_ref().map(AsRef::as_ref) as Option<&str>,
            item.batch_number.as_deref(),
            item.quantity_on_hand,
            item.quantity_allocated,
            Uuid::from(item.quantity_unit_id),
            item.location_id.map(Uuid::from),
            item.status.as_str(),
            item.public_notes.as_deref(),
            item.internal_notes.as_deref(),
        )
        .fetch_one(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
        rows.push(row);
    }

    Ok(rows)
}

pub(super) async fn convert_inventory_items_to_unit(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    target_unit_id: Uuid,
) -> Result<(), AssetDatabaseError> {
    let items = fetch_inventory_items_for_asset_for_update(transaction, asset_id).await?;

    for item in items {
        if item.quantity_unit_id == target_unit_id {
            continue;
        }

        let quantity_on_hand = convert_quantity_between_units(
            transaction,
            item.quantity_unit_id,
            target_unit_id,
            item.quantity_on_hand,
        )
        .await?;
        let quantity_allocated = convert_quantity_between_units(
            transaction,
            item.quantity_unit_id,
            target_unit_id,
            item.quantity_allocated,
        )
        .await?;

        sqlx::query!(
            r#"
            UPDATE asset_inventory_items
            SET
                quantity_on_hand = $2::double precision::numeric,
                quantity_allocated = $3::double precision::numeric,
                quantity_unit_id = $4,
                updated_at = now()
            WHERE inventory_item_id = $1
            "#,
            item.inventory_item_id,
            quantity_on_hand,
            quantity_allocated,
            target_unit_id,
        )
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
    }

    Ok(())
}

async fn convert_quantity_between_units(
    transaction: &mut Transaction<'_, Postgres>,
    source_unit_id: Uuid,
    target_unit_id: Uuid,
    source_quantity: f64,
) -> Result<f64, AssetDatabaseError> {
    if source_unit_id == target_unit_id {
        return Ok(source_quantity);
    }
    let source_unit = fetch_unit(transaction, source_unit_id).await?;
    let target_unit = fetch_unit(transaction, target_unit_id).await?;
    if source_unit.dimension != target_unit.dimension {
        return Err(AssetDatabaseError::Validation(
            "Asset default unit dimension does not match inventory item unit dimension".into(),
        ));
    }

    Ok(source_quantity * source_unit.scale_to_base / target_unit.scale_to_base)
}

async fn validate_location(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    location_id: Uuid,
) -> Result<(), AssetDatabaseError> {
    let found = sqlx::query_scalar!(
        r#"
        SELECT location_id
        FROM locations
        WHERE laboratory_id = $1
          AND location_id = $2
        FOR UPDATE
        "#,
        *laboratory_id,
        location_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch location")?;

    if found.is_some() {
        Ok(())
    } else {
        Err(AssetDatabaseError::Validation(
            "Inventory item location does not belong to this laboratory".into(),
        ))
    }
}

pub(super) async fn apply_asset_parameter_updates(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
    inputs: &[AssetParameterValueInput],
    allow_delete: bool,
) -> Result<(), AssetDatabaseError> {
    validate_unique_parameter_inputs(inputs)?;
    let definitions = fetch_parameter_definitions(transaction, laboratory_id, inputs).await?;
    for input in inputs {
        match input.value.as_ref() {
            Some(value) => {
                let definition = definitions.get(&input.parameter_type_id).ok_or(
                    AssetDatabaseError::Validation(
                        "Asset parameter does not belong to this laboratory".into(),
                    ),
                )?;
                let resolved = resolve_parameter_value(transaction, definition, value).await?;
                upsert_asset_parameter_value(transaction, laboratory_id, asset_id, &resolved)
                    .await?;
            }
            None => {
                if !allow_delete {
                    return Err(AssetDatabaseError::Validation(
                        "Asset parameter value is required".into(),
                    ));
                }
                delete_asset_parameter_value(transaction, asset_id, input.parameter_type_id)
                    .await?;
            }
        }
    }

    Ok(())
}

fn validate_unique_parameter_inputs(
    inputs: &[AssetParameterValueInput],
) -> Result<(), AssetDatabaseError> {
    let mut seen = HashSet::new();
    for input in inputs {
        if !seen.insert(input.parameter_type_id) {
            return Err(AssetDatabaseError::Validation(
                "Asset parameter values must be unique".into(),
            ));
        }
    }

    Ok(())
}

async fn fetch_parameter_definitions(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    inputs: &[AssetParameterValueInput],
) -> Result<HashMap<Uuid, AssetParameterDefinitionRow>, AssetDatabaseError> {
    let parameter_type_ids: Vec<_> = inputs.iter().map(|input| input.parameter_type_id).collect();
    if parameter_type_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as!(
        AssetParameterDefinitionRow,
        r#"
        SELECT
            parameter_type_id,
            data_type::text AS "data_type!",
            unit_dimension,
            default_unit_id
        FROM asset_parameter_types
        WHERE laboratory_id = $1
          AND parameter_type_id = ANY($2)
        FOR UPDATE
        "#,
        *laboratory_id,
        &parameter_type_ids,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter definitions")?;

    Ok(rows
        .into_iter()
        .map(|row| (row.parameter_type_id, row))
        .collect())
}

async fn resolve_parameter_value(
    transaction: &mut Transaction<'_, Postgres>,
    definition: &AssetParameterDefinitionRow,
    value: &Value,
) -> Result<ResolvedAssetParameterValue, AssetDatabaseError> {
    let data_type = AssetParameterDataType::parse(&definition.data_type)
        .map_err(|_| AssetDatabaseError::Validation("Invalid asset parameter data type".into()))?;
    let value =
        AssetParameterValue::parse(data_type, value).map_err(AssetDatabaseError::Validation)?;
    let mut resolved = ResolvedAssetParameterValue {
        parameter_type_id: definition.parameter_type_id,
        data_type: definition.data_type.clone(),
        value_text: None,
        value_number: None,
        value_number_base: None,
        value_range_start: None,
        value_range_end: None,
        value_range_start_base: None,
        value_range_end_base: None,
        unit_id: None,
        value_boolean: None,
        value_date: None,
        value_option_id: None,
    };

    match value {
        AssetParameterValue::Text(text) => resolved.value_text = Some(text),
        AssetParameterValue::Number { number, unit_id } => {
            let (unit_id, number_base) =
                normalize_unit_value(transaction, definition, unit_id.map(Uuid::from), number)
                    .await?;
            resolved.value_number = Some(number);
            resolved.value_number_base = number_base;
            resolved.unit_id = unit_id;
        }
        AssetParameterValue::Range {
            start,
            end,
            unit_id,
        } => {
            let (unit_id, start_base) =
                normalize_unit_value(transaction, definition, unit_id.map(Uuid::from), start)
                    .await?;
            let (_, end_base) = normalize_unit_value(transaction, definition, unit_id, end).await?;
            resolved.value_range_start = Some(start);
            resolved.value_range_end = Some(end);
            resolved.value_range_start_base = start_base;
            resolved.value_range_end_base = end_base;
            resolved.unit_id = unit_id;
        }
        AssetParameterValue::Boolean(boolean) => resolved.value_boolean = Some(boolean),
        AssetParameterValue::Date(date) => resolved.value_date = Some(date),
        AssetParameterValue::Enum(option_id) => {
            validate_option(transaction, definition.parameter_type_id, option_id).await?;
            resolved.value_option_id = Some(option_id);
        }
    }

    Ok(resolved)
}

async fn normalize_unit_value(
    transaction: &mut Transaction<'_, Postgres>,
    definition: &AssetParameterDefinitionRow,
    unit_id: Option<Uuid>,
    value: f64,
) -> Result<(Option<Uuid>, Option<f64>), AssetDatabaseError> {
    let unit_id = unit_id.or(definition.default_unit_id);
    let Some(unit_id) = unit_id else {
        return Ok((None, None));
    };
    let unit = fetch_unit(transaction, unit_id).await?;

    match definition.unit_dimension.as_deref() {
        Some(unit_dimension) if unit_dimension == unit.dimension => {
            Ok((Some(unit.unit_id), Some(value * unit.scale_to_base)))
        }
        Some(_) => Err(AssetDatabaseError::Validation(
            "Parameter value unit dimension does not match parameter definition".into(),
        )),
        None => Err(AssetDatabaseError::Validation(
            "Parameter value unit is not allowed for this parameter".into(),
        )),
    }
}

async fn fetch_unit(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> Result<UnitRow, AssetDatabaseError> {
    sqlx::query_as!(
        UnitRow,
        r#"
        SELECT unit_id, dimension, scale_to_base
        FROM units
        WHERE unit_id = $1
        FOR UPDATE
        "#,
        unit_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch unit")?
    .ok_or(AssetDatabaseError::Validation("Unit not found".into()))
}

async fn validate_option(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_type_id: Uuid,
    option_id: Uuid,
) -> Result<(), AssetDatabaseError> {
    let found = sqlx::query_scalar!(
        r#"
        SELECT option_id
        FROM asset_parameter_options
        WHERE parameter_type_id = $1
          AND option_id = $2
        FOR UPDATE
        "#,
        parameter_type_id,
        option_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter option")?;

    if found.is_some() {
        Ok(())
    } else {
        Err(AssetDatabaseError::Validation(
            "Asset parameter option not found".into(),
        ))
    }
}

async fn upsert_asset_parameter_value(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
    value: &ResolvedAssetParameterValue,
) -> Result<(), AssetDatabaseError> {
    sqlx::query!(
        r#"
        INSERT INTO asset_parameter_values (
            value_id,
            laboratory_id,
            asset_id,
            parameter_type_id,
            data_type,
            value_text,
            value_number,
            value_number_base,
            value_range_start,
            value_range_end,
            value_range_start_base,
            value_range_end_base,
            unit_id,
            value_boolean,
            value_date,
            value_option_id
        )
        VALUES ($1, $2, $3, $4, $5::text::asset_parameter_data_type, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
        ON CONFLICT (asset_id, parameter_type_id)
        DO UPDATE SET
            data_type = EXCLUDED.data_type,
            value_text = EXCLUDED.value_text,
            value_number = EXCLUDED.value_number,
            value_number_base = EXCLUDED.value_number_base,
            value_range_start = EXCLUDED.value_range_start,
            value_range_end = EXCLUDED.value_range_end,
            value_range_start_base = EXCLUDED.value_range_start_base,
            value_range_end_base = EXCLUDED.value_range_end_base,
            unit_id = EXCLUDED.unit_id,
            value_boolean = EXCLUDED.value_boolean,
            value_date = EXCLUDED.value_date,
            value_option_id = EXCLUDED.value_option_id,
            updated_at = now()
        "#,
        Uuid::new_v4(),
        *laboratory_id,
        asset_id,
        value.parameter_type_id,
        &value.data_type,
        value.value_text.as_deref(),
        value.value_number,
        value.value_number_base,
        value.value_range_start,
        value.value_range_end,
        value.value_range_start_base,
        value.value_range_end_base,
        value.unit_id,
        value.value_boolean,
        value.value_date,
        value.value_option_id,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

async fn delete_asset_parameter_value(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    parameter_type_id: Uuid,
) -> Result<(), AssetDatabaseError> {
    sqlx::query!(
        r#"
        DELETE FROM asset_parameter_values
        WHERE asset_id = $1
          AND parameter_type_id = $2
        "#,
        asset_id,
        parameter_type_id,
    )
    .execute(transaction.as_mut())
    .await
    .context("Failed to delete asset parameter value")?;

    Ok(())
}

pub(super) async fn validate_required_parameters(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
    category_id: Option<Uuid>,
) -> Result<(), AssetDatabaseError> {
    let Some(category_id) = category_id else {
        return Ok(());
    };
    let required = fetch_required_parameters(transaction, laboratory_id, category_id).await?;
    if required.is_empty() {
        return Ok(());
    }

    let existing: HashSet<Uuid> = sqlx::query_scalar!(
        r#"
        SELECT parameter_type_id
        FROM asset_parameter_values
        WHERE laboratory_id = $1
          AND asset_id = $2
        "#,
        *laboratory_id,
        asset_id,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter values")?
    .into_iter()
    .collect();

    for parameter_type_id in required {
        if !existing.contains(&parameter_type_id) {
            return Err(AssetDatabaseError::Validation(
                "Missing required asset parameter value".into(),
            ));
        }
    }

    Ok(())
}

async fn fetch_required_parameters(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    category_id: Uuid,
) -> Result<Vec<Uuid>, AssetDatabaseError> {
    sqlx::query_scalar!(
        r#"
        SELECT DISTINCT ON (assignments.parameter_type_id)
            assignments.parameter_type_id
        FROM asset_categories AS current_category
        JOIN asset_categories AS ancestor_category
          ON ancestor_category.laboratory_id = current_category.laboratory_id
         AND ancestor_category.path @> current_category.path
        JOIN asset_parameter_assignments AS assignments
          ON assignments.category_id = ancestor_category.category_id
        WHERE current_category.laboratory_id = $1
          AND current_category.category_id = $2
          AND assignments.is_required = true
          AND (
              ancestor_category.category_id = current_category.category_id
              OR assignments.applies_to_descendants = true
          )
        ORDER BY assignments.parameter_type_id, ancestor_category.depth DESC, assignments.sort_order
        "#,
        *laboratory_id,
        category_id,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch required asset parameters")
    .map_err(AssetDatabaseError::Unexpected)
}

pub(super) async fn delete_asset_attachments(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    inventory_item_ids: &[Uuid],
) -> Result<Vec<DeletedAttachmentRow>, AssetDatabaseError> {
    sqlx::query_as!(
        DeletedAttachmentRow,
        r#"
        WITH deleted_assignments AS (
            DELETE FROM asset_attachment_assignments
            WHERE (
                  asset_id = $1
                  OR inventory_item_id = ANY($2)
              )
            RETURNING attachment_id, file_id
        ),
        deleted_files AS (
            DELETE FROM files
            WHERE file_id IN (SELECT file_id FROM deleted_assignments)
            RETURNING file_id, storage_key
        )
        SELECT deleted_assignments.attachment_id, deleted_files.storage_key
        FROM deleted_assignments
        JOIN deleted_files ON deleted_files.file_id = deleted_assignments.file_id
        ORDER BY deleted_assignments.attachment_id
        "#,
        asset_id,
        inventory_item_ids,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to delete asset attachments")
    .map_err(AssetDatabaseError::Unexpected)
}

pub(super) fn map_database_error(error: sqlx::Error) -> AssetDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("idx_assets_unique_laboratory_name_model")) => {
                return AssetDatabaseError::Conflict(
                    "Asset name and model already exist in this laboratory".into(),
                );
            }
            (Some("23505"), Some("idx_asset_inventory_items_unique_asset_serial_number")) => {
                return AssetDatabaseError::Conflict(
                    "Inventory item serial number already exists for this asset".into(),
                );
            }
            (Some("23505"), Some("idx_asset_inventory_items_unique_quantity_aggregate")) => {
                return AssetDatabaseError::Conflict(
                    "Inventory item already exists for this quantity aggregate".into(),
                );
            }
            (Some("23505"), _) => {
                return AssetDatabaseError::Conflict("Asset already exists".into());
            }
            (Some("23503"), _) => {
                return AssetDatabaseError::Validation("Invalid referenced record".into());
            }
            (Some("23514"), _) => {
                return AssetDatabaseError::Validation("Invalid asset data".into());
            }
            _ => {}
        }
    }

    AssetDatabaseError::Unexpected(error.into())
}

fn parameter_value_json(row: &AssetParameterValueRow) -> Value {
    match row.data_type.as_str() {
        "text" => json!({ "text": row.value_text }),
        "number" => json!({
            "number": row.value_number,
            "number_base": row.value_number_base,
            "unit_id": row.unit_id,
        }),
        "range" => json!({
            "range_start": row.value_range_start,
            "range_end": row.value_range_end,
            "range_start_base": row.value_range_start_base,
            "range_end_base": row.value_range_end_base,
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
