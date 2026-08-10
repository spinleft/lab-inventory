//! Every SQL statement the asset routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - functions never return a handler error type, only [`AssetDatabaseError`],
//!   so any handler can reuse them
use super::model::{
    AssetInventoryItemRow, AssetParameterDefinitionRow, AssetParameterValueRow, AssetRow,
    DeletedAttachmentRow, ResolvedAssetParameterValue, UnitRow,
};
use crate::domain::{AssetTrackingMode, LaboratoryId, NewAsset, NewInventoryItem};
use crate::utils::error_chain_fmt;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

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

/// The projection shared with [`fetch_asset`], for `QueryBuilder` callers that
/// need to append their own filters.
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
        assets.inventory_unit_id,
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

// ---------------------------------------------------------------------------
// assets
// ---------------------------------------------------------------------------

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
            assets.inventory_unit_id,
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

/// Same projection as [`fetch_asset`], but takes the row lock the write paths
/// need. `query_as!` requires a literal, so the column list cannot be shared.
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
            assets.inventory_unit_id,
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

#[tracing::instrument(
    name = "Saving new asset in the database",
    skip(transaction, new_asset),
    fields(laboratory_id=%laboratory_id)
)]
pub(super) async fn insert_asset(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    new_asset: &NewAsset,
) -> Result<AssetRow, AssetDatabaseError> {
    sqlx::query_as!(
        AssetRow,
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            category_id,
            tracking_mode,
            name,
            model,
            manufacturer,
            inventory_unit_id,
            public_notes,
            internal_notes
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING
            asset_id,
            laboratory_id,
            category_id,
            tracking_mode,
            name,
            model,
            manufacturer,
            inventory_unit_id,
            public_notes,
            internal_notes,
            created_at,
            updated_at,
            0::bigint AS "inventory_item_count!",
            0::double precision AS "quantity_on_hand!",
            0::double precision AS "quantity_allocated!"
        "#,
        Uuid::new_v4(),
        *laboratory_id,
        new_asset.category_id.map(Uuid::from),
        new_asset.tracking_mode.as_str(),
        new_asset.name.as_ref(),
        new_asset.model.as_deref(),
        new_asset.manufacturer.as_deref(),
        Uuid::from(new_asset.inventory_unit_id),
        new_asset.public_notes.as_deref(),
        new_asset.internal_notes.as_deref(),
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Updating asset in the database",
    skip(transaction, name, model, manufacturer, public_notes, internal_notes),
    fields(asset_id=%asset_id)
)]
pub(super) async fn update_asset_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    category_id: Option<Uuid>,
    tracking_mode: AssetTrackingMode,
    name: &str,
    model: Option<&str>,
    manufacturer: Option<&str>,
    inventory_unit_id: Uuid,
    public_notes: Option<&str>,
    internal_notes: Option<&str>,
) -> Result<(), AssetDatabaseError> {
    sqlx::query!(
        r#"
        UPDATE assets
        SET
            category_id = $2,
            tracking_mode = $3,
            name = $4,
            model = $5,
            manufacturer = $6,
            inventory_unit_id = $7,
            public_notes = $8,
            internal_notes = $9,
            updated_at = now()
        WHERE asset_id = $1
        "#,
        asset_id,
        category_id,
        tracking_mode.as_str(),
        name,
        model,
        manufacturer,
        inventory_unit_id,
        public_notes,
        internal_notes,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

#[tracing::instrument(
    name = "Deleting asset from the database",
    skip(transaction),
    fields(asset_id=%asset_id)
)]
pub(super) async fn delete_asset_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
) -> Result<(), AssetDatabaseError> {
    sqlx::query!(
        r#"
        DELETE FROM assets
        WHERE asset_id = $1
        "#,
        asset_id,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// inventory items
// ---------------------------------------------------------------------------

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

pub(super) async fn insert_inventory_item(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
    tracking_mode: AssetTrackingMode,
    item: &NewInventoryItem,
) -> Result<AssetInventoryItemRow, AssetDatabaseError> {
    sqlx::query_as!(
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
            location_id,
            status,
            public_notes,
            internal_notes
        )
        VALUES (
            $1, $2, $3, $4, $5, $6,
            $7::double precision::numeric,
            $8::double precision::numeric,
            $9, $10, $11, $12
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
        item.location_id.map(Uuid::from),
        item.status.as_str(),
        item.public_notes.as_deref(),
        item.internal_notes.as_deref(),
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

/// Multiplies every stored quantity of an asset by `factor`, used when the
/// asset's inventory unit changes and the recorded amounts have to keep meaning
/// the same physical quantity.
pub(super) async fn rescale_inventory_quantities(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    factor: f64,
) -> Result<(), AssetDatabaseError> {
    sqlx::query!(
        r#"
        UPDATE asset_inventory_items
        SET
            quantity_on_hand = quantity_on_hand * $2::double precision::numeric,
            quantity_allocated = quantity_allocated * $2::double precision::numeric,
            updated_at = now()
        WHERE asset_id = $1
        "#,
        asset_id,
        factor,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// parameter values
// ---------------------------------------------------------------------------

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
            asset_parameter_values.value_number_in_base,
            asset_parameter_values.value_range_start,
            asset_parameter_values.value_range_end,
            asset_parameter_values.value_range_start_in_base,
            asset_parameter_values.value_range_end_in_base,
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
            asset_parameter_values.value_number_in_base,
            asset_parameter_values.value_range_start,
            asset_parameter_values.value_range_end,
            asset_parameter_values.value_range_start_in_base,
            asset_parameter_values.value_range_end_in_base,
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
            asset_parameter_values.value_number_in_base,
            asset_parameter_values.value_range_start,
            asset_parameter_values.value_range_end,
            asset_parameter_values.value_range_start_in_base,
            asset_parameter_values.value_range_end_in_base,
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

pub(super) async fn fetch_parameter_definitions(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    parameter_type_ids: &[Uuid],
) -> Result<HashMap<Uuid, AssetParameterDefinitionRow>, AssetDatabaseError> {
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
        parameter_type_ids,
    )
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter definitions")?;

    Ok(rows
        .into_iter()
        .map(|row| (row.parameter_type_id, row))
        .collect())
}

pub(super) async fn upsert_asset_parameter_value(
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
            value_number_in_base,
            value_range_start,
            value_range_end,
            value_range_start_in_base,
            value_range_end_in_base,
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
            value_number_in_base = EXCLUDED.value_number_in_base,
            value_range_start = EXCLUDED.value_range_start,
            value_range_end = EXCLUDED.value_range_end,
            value_range_start_in_base = EXCLUDED.value_range_start_in_base,
            value_range_end_in_base = EXCLUDED.value_range_end_in_base,
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
        value.value_number_in_base,
        value.value_range_start,
        value.value_range_end,
        value.value_range_start_in_base,
        value.value_range_end_in_base,
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

pub(super) async fn delete_asset_parameter_value(
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

pub(super) async fn fetch_parameter_type_ids_with_values(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    asset_id: Uuid,
) -> Result<HashSet<Uuid>, AssetDatabaseError> {
    let ids = sqlx::query_scalar!(
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
    .context("Failed to fetch asset parameter values")?;

    Ok(ids.into_iter().collect())
}

pub(super) async fn fetch_required_parameters(
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

// ---------------------------------------------------------------------------
// existence checks and lookups
// ---------------------------------------------------------------------------

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

pub(super) async fn validate_location(
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

pub(super) async fn validate_option(
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

pub(super) async fn fetch_unit(
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

// ---------------------------------------------------------------------------
// attachments
// ---------------------------------------------------------------------------

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
