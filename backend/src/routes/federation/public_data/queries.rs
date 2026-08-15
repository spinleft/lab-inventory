//! Every SQL statement the federation public read API issues lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - every statement is scoped to one laboratory and to what is publicly
//!   shareable: no internal notes, and attachments only where `is_public`
use super::model::{
    AssetPublicRow, AttachmentDownloadRow, AttachmentPublicRow, CategoryRow,
    InventoryItemPublicRow, LaboratoryPublicRow, LocationRow, ParameterOptionRow, ParameterRow,
    ParameterValueRow, UnitRow,
};
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::collections::HashMap;
use uuid::Uuid;

/// Every statement here is a read that can only fail unexpectedly, so a row that
/// is not there comes back as `None` and `service.rs` decides what missing means.
fn unexpected(error: sqlx::Error) -> anyhow::Error {
    anyhow::Error::from(error).context("Failed to read federation public data")
}

// ---------------------------------------------------------------------------
// laboratory
// ---------------------------------------------------------------------------

pub(super) async fn fetch_laboratory(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Option<LaboratoryPublicRow>, anyhow::Error> {
    sqlx::query_as::<_, LaboratoryPublicRow>(
        r#"
        SELECT laboratory_id, name, address, description, contact, created_at, updated_at
        FROM laboratories
        WHERE laboratory_id = $1
        "#,
    )
    .bind(laboratory_id)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)
}

// ---------------------------------------------------------------------------
// assets
// ---------------------------------------------------------------------------

/// The projection every public asset read shares. It deliberately omits
/// `internal_notes`.
fn asset_select() -> &'static str {
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

pub(super) async fn fetch_assets(
    pool: &PgPool,
    laboratory_id: Uuid,
    params: &HashMap<String, String>,
    limit: i64,
    offset: i64,
) -> Result<Vec<AssetPublicRow>, anyhow::Error> {
    let mut builder = QueryBuilder::<Postgres>::new(asset_select());
    push_asset_filters(&mut builder, laboratory_id, params);
    builder.push(" ORDER BY assets.updated_at DESC, assets.asset_id LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    builder
        .build_query_as::<AssetPublicRow>()
        .fetch_all(pool)
        .await
        .map_err(unexpected)
}

pub(super) async fn count_assets(
    pool: &PgPool,
    laboratory_id: Uuid,
    params: &HashMap<String, String>,
) -> Result<i64, anyhow::Error> {
    let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM assets");
    push_asset_filters(&mut builder, laboratory_id, params);

    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(unexpected)
}

fn push_asset_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    laboratory_id: Uuid,
    params: &HashMap<String, String>,
) {
    builder.push(" WHERE assets.laboratory_id = ");
    builder.push_bind(laboratory_id);
    if let Some(keyword) = params
        .get("keyword")
        .map(String::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let pattern = format!("%{keyword}%");
        builder.push(" AND (assets.name ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(assets.model, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(assets.manufacturer, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(assets.public_notes, '') ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    if let Some(category_id) = params
        .get("category_id")
        .and_then(|value| value.parse::<Uuid>().ok())
    {
        builder.push(" AND assets.category_id = ");
        builder.push_bind(category_id);
    }
    if let Some(tracking_mode) = params
        .get("tracking_mode")
        .map(String::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        builder.push(" AND assets.tracking_mode = ");
        builder.push_bind(tracking_mode.to_string());
    }
    if let Some(manufacturer) = params
        .get("manufacturer")
        .map(String::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        builder.push(" AND assets.manufacturer = ");
        builder.push_bind(manufacturer.to_string());
    }
}

pub(super) async fn fetch_asset(
    pool: &PgPool,
    laboratory_id: Uuid,
    asset_id: Uuid,
) -> Result<Option<AssetPublicRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetPublicRow>(&format!(
        "{} WHERE assets.laboratory_id = $1 AND assets.asset_id = $2",
        asset_select()
    ))
    .bind(laboratory_id)
    .bind(asset_id)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)
}

// ---------------------------------------------------------------------------
// inventory items
// ---------------------------------------------------------------------------

/// The projection every public inventory read shares. It deliberately omits
/// `internal_notes`.
fn inventory_item_select() -> &'static str {
    r#"
    SELECT
        asset_inventory_items.inventory_item_id,
        asset_inventory_items.asset_id,
        asset_inventory_items.laboratory_id,
        asset_inventory_items.tracking_mode,
        asset_inventory_items.serial_number,
        asset_inventory_items.batch_number,
        asset_inventory_items.quantity_on_hand::double precision AS quantity_on_hand,
        asset_inventory_items.quantity_allocated::double precision AS quantity_allocated,
        asset_inventory_items.location_id,
        asset_inventory_items.status,
        asset_inventory_items.public_notes,
        asset_inventory_items.created_at,
        asset_inventory_items.updated_at,
        asset_inventory_items.last_stocktake_at,
        assets.category_id AS asset_category_id,
        assets.name AS asset_name,
        assets.model AS asset_model,
        assets.manufacturer AS asset_manufacturer,
        assets.inventory_unit_id AS asset_inventory_unit_id
    FROM asset_inventory_items
    JOIN assets ON assets.asset_id = asset_inventory_items.asset_id
    "#
}

pub(super) async fn fetch_inventory_items(
    pool: &PgPool,
    laboratory_id: Uuid,
    params: &HashMap<String, String>,
    limit: i64,
    offset: i64,
) -> Result<Vec<InventoryItemPublicRow>, anyhow::Error> {
    let mut builder = QueryBuilder::<Postgres>::new(inventory_item_select());
    push_inventory_filters(&mut builder, laboratory_id, params);
    builder.push(" ORDER BY asset_inventory_items.updated_at DESC, asset_inventory_items.inventory_item_id LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    builder
        .build_query_as::<InventoryItemPublicRow>()
        .fetch_all(pool)
        .await
        .map_err(unexpected)
}

pub(super) async fn count_inventory_items(
    pool: &PgPool,
    laboratory_id: Uuid,
    params: &HashMap<String, String>,
) -> Result<i64, anyhow::Error> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*) FROM asset_inventory_items JOIN assets ON assets.asset_id = asset_inventory_items.asset_id",
    );
    push_inventory_filters(&mut builder, laboratory_id, params);

    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(unexpected)
}

fn push_inventory_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    laboratory_id: Uuid,
    params: &HashMap<String, String>,
) {
    builder.push(" WHERE asset_inventory_items.laboratory_id = ");
    builder.push_bind(laboratory_id);
    if let Some(asset_id) = params
        .get("asset_id")
        .and_then(|value| value.parse::<Uuid>().ok())
    {
        builder.push(" AND asset_inventory_items.asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(status) = params
        .get("status")
        .map(String::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        builder.push(" AND asset_inventory_items.status = ");
        builder.push_bind(status.to_string());
    }
    if let Some(keyword) = params
        .get("keyword")
        .map(String::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let pattern = format!("%{keyword}%");
        builder.push(" AND (assets.name ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(asset_inventory_items.serial_number, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(asset_inventory_items.batch_number, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(asset_inventory_items.public_notes, '') ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
}

pub(super) async fn fetch_inventory_item(
    pool: &PgPool,
    laboratory_id: Uuid,
    inventory_item_id: Uuid,
) -> Result<Option<InventoryItemPublicRow>, anyhow::Error> {
    sqlx::query_as::<_, InventoryItemPublicRow>(&format!(
        "{} WHERE asset_inventory_items.laboratory_id = $1 AND asset_inventory_items.inventory_item_id = $2",
        inventory_item_select()
    ))
    .bind(laboratory_id)
    .bind(inventory_item_id)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_inventory_items_for_asset(
    pool: &PgPool,
    laboratory_id: Uuid,
    asset_id: Uuid,
) -> Result<Vec<InventoryItemPublicRow>, anyhow::Error> {
    sqlx::query_as::<_, InventoryItemPublicRow>(&format!(
        "{} WHERE asset_inventory_items.laboratory_id = $1 AND asset_inventory_items.asset_id = $2 ORDER BY asset_inventory_items.created_at, asset_inventory_items.inventory_item_id",
        inventory_item_select()
    ))
    .bind(laboratory_id)
    .bind(asset_id)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

// ---------------------------------------------------------------------------
// categories and locations
// ---------------------------------------------------------------------------

pub(super) async fn fetch_categories(
    pool: &PgPool,
    laboratory_id: Uuid,
    root_path: Option<String>,
) -> Result<Vec<CategoryRow>, anyhow::Error> {
    sqlx::query_as::<_, CategoryRow>(
        r#"
        SELECT category_id, laboratory_id, parent_category_id, name, code, path::text AS path, depth, description, created_at, updated_at
        FROM asset_categories
        WHERE laboratory_id = $1
          AND ($2::text IS NULL OR path <@ $2::text::ltree)
        ORDER BY path
        "#,
    )
    .bind(laboratory_id)
    .bind(root_path)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_category(
    pool: &PgPool,
    laboratory_id: Uuid,
    category_id: Uuid,
) -> Result<Option<CategoryRow>, anyhow::Error> {
    sqlx::query_as::<_, CategoryRow>(
        r#"
        SELECT category_id, laboratory_id, parent_category_id, name, code, path::text AS path, depth, description, created_at, updated_at
        FROM asset_categories
        WHERE laboratory_id = $1 AND category_id = $2
        "#,
    )
    .bind(laboratory_id)
    .bind(category_id)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_locations(
    pool: &PgPool,
    laboratory_id: Uuid,
    root_path: Option<String>,
) -> Result<Vec<LocationRow>, anyhow::Error> {
    sqlx::query_as::<_, LocationRow>(
        r#"
        SELECT location_id, laboratory_id, parent_location_id, name, code, path::text AS path, depth, description, created_at, updated_at
        FROM locations
        WHERE laboratory_id = $1
          AND ($2::text IS NULL OR path <@ $2::text::ltree)
        ORDER BY path
        "#,
    )
    .bind(laboratory_id)
    .bind(root_path)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_location(
    pool: &PgPool,
    laboratory_id: Uuid,
    location_id: Uuid,
) -> Result<Option<LocationRow>, anyhow::Error> {
    sqlx::query_as::<_, LocationRow>(
        r#"
        SELECT location_id, laboratory_id, parent_location_id, name, code, path::text AS path, depth, description, created_at, updated_at
        FROM locations
        WHERE laboratory_id = $1 AND location_id = $2
        "#,
    )
    .bind(laboratory_id)
    .bind(location_id)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_units(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Vec<UnitRow>, anyhow::Error> {
    sqlx::query_as::<_, UnitRow>(
        r#"
        SELECT unit_id, laboratory_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        FROM units
        WHERE laboratory_id = $1
        ORDER BY dimension, code
        "#,
    )
    .bind(laboratory_id)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_unit(
    pool: &PgPool,
    laboratory_id: Uuid,
    unit_id: Uuid,
) -> Result<Option<UnitRow>, anyhow::Error> {
    sqlx::query_as::<_, UnitRow>(
        r#"
        SELECT unit_id, laboratory_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        FROM units
        WHERE laboratory_id = $1 AND unit_id = $2
        "#,
    )
    .bind(laboratory_id)
    .bind(unit_id)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)
}

// ---------------------------------------------------------------------------
// parameters
// ---------------------------------------------------------------------------

pub(super) async fn fetch_parameters(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Vec<ParameterRow>, anyhow::Error> {
    sqlx::query_as::<_, ParameterRow>(
        r#"
        SELECT parameter_type_id, laboratory_id, code, name, data_type::text AS data_type, unit_dimension, default_unit_id, description, created_at, updated_at
        FROM asset_parameter_types
        WHERE laboratory_id = $1
        ORDER BY code
        "#,
    )
    .bind(laboratory_id)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_parameter(
    pool: &PgPool,
    laboratory_id: Uuid,
    parameter_id: Uuid,
) -> Result<Option<ParameterRow>, anyhow::Error> {
    sqlx::query_as::<_, ParameterRow>(
        r#"
        SELECT parameter_type_id, laboratory_id, code, name, data_type::text AS data_type, unit_dimension, default_unit_id, description, created_at, updated_at
        FROM asset_parameter_types
        WHERE laboratory_id = $1 AND parameter_type_id = $2
        "#,
    )
    .bind(laboratory_id)
    .bind(parameter_id)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_parameter_options(
    pool: &PgPool,
    parameter_type_id: Uuid,
) -> Result<Vec<ParameterOptionRow>, anyhow::Error> {
    sqlx::query_as::<_, ParameterOptionRow>(
        r#"
        SELECT option_id, parameter_type_id, code, label, sort_order
        FROM asset_parameter_options
        WHERE parameter_type_id = $1
        ORDER BY sort_order, label, code
        "#,
    )
    .bind(parameter_type_id)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_parameter_values(
    pool: &PgPool,
    asset_ids: &[Uuid],
) -> Result<Vec<ParameterValueRow>, anyhow::Error> {
    if asset_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_as::<_, ParameterValueRow>(
        r#"
        SELECT
            asset_parameter_values.value_id,
            asset_parameter_values.laboratory_id,
            asset_parameter_values.asset_id,
            asset_parameter_values.parameter_type_id,
            asset_parameter_types.code,
            asset_parameter_types.name,
            asset_parameter_values.data_type::text AS data_type,
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
            asset_parameter_options.code AS option_code,
            asset_parameter_options.label AS option_label,
            asset_parameter_values.created_at,
            asset_parameter_values.updated_at
        FROM asset_parameter_values
        JOIN asset_parameter_types ON asset_parameter_types.parameter_type_id = asset_parameter_values.parameter_type_id
        LEFT JOIN asset_parameter_options ON asset_parameter_options.option_id = asset_parameter_values.value_option_id
        WHERE asset_parameter_values.asset_id = ANY($1)
        ORDER BY asset_parameter_values.asset_id, asset_parameter_types.name, asset_parameter_types.code
        "#,
    )
    .bind(asset_ids)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

// ---------------------------------------------------------------------------
// attachments
// ---------------------------------------------------------------------------

fn attachment_select(suffix: &str) -> String {
    format!(
        r#"
        SELECT
            assignments.attachment_id,
            assignments.laboratory_id,
            assignments.asset_id,
            assignments.inventory_item_id,
            assignments.display_name,
            files.original_file_name,
            assignments.description,
            files.mime_type,
            files.file_size_bytes,
            files.sha256_hex,
            assignments.is_public,
            files.uploaded_by_user_id,
            assignments.created_at,
            assignments.updated_at
        FROM asset_attachment_assignments AS assignments
        JOIN files ON files.file_id = assignments.file_id
        {suffix}
        "#
    )
}

pub(super) async fn count_laboratory_attachments(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<i64, anyhow::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM asset_attachment_assignments AS assignments
        WHERE assignments.laboratory_id = $1
          AND assignments.is_public
        "#,
    )
    .bind(laboratory_id)
    .fetch_one(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_laboratory_attachments(
    pool: &PgPool,
    laboratory_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<AttachmentPublicRow>, anyhow::Error> {
    sqlx::query_as::<_, AttachmentPublicRow>(&attachment_select(
        "WHERE assignments.laboratory_id = $1 AND assignments.is_public ORDER BY assignments.created_at DESC, assignments.attachment_id LIMIT $2 OFFSET $3",
    ))
    .bind(laboratory_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_asset_attachments(
    pool: &PgPool,
    laboratory_id: Uuid,
    asset_id: Uuid,
) -> Result<Vec<AttachmentPublicRow>, anyhow::Error> {
    sqlx::query_as::<_, AttachmentPublicRow>(&attachment_select(
        "WHERE assignments.laboratory_id = $1 AND assignments.asset_id = $2 AND assignments.is_public ORDER BY assignments.created_at DESC, assignments.attachment_id",
    ))
    .bind(laboratory_id)
    .bind(asset_id)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_inventory_item_attachments(
    pool: &PgPool,
    laboratory_id: Uuid,
    inventory_item_id: Uuid,
) -> Result<Vec<AttachmentPublicRow>, anyhow::Error> {
    sqlx::query_as::<_, AttachmentPublicRow>(&attachment_select(
        "WHERE assignments.laboratory_id = $1 AND assignments.inventory_item_id = $2 AND assignments.is_public ORDER BY assignments.created_at DESC, assignments.attachment_id",
    ))
    .bind(laboratory_id)
    .bind(inventory_item_id)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_attachment(
    pool: &PgPool,
    laboratory_id: Uuid,
    attachment_id: Uuid,
) -> Result<Option<AttachmentPublicRow>, anyhow::Error> {
    sqlx::query_as::<_, AttachmentPublicRow>(&attachment_select(
        "WHERE assignments.laboratory_id = $1 AND assignments.attachment_id = $2 AND assignments.is_public",
    ))
    .bind(laboratory_id)
    .bind(attachment_id)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_attachment_download(
    pool: &PgPool,
    laboratory_id: Uuid,
    attachment_id: Uuid,
) -> Result<Option<AttachmentDownloadRow>, anyhow::Error> {
    sqlx::query_as::<_, AttachmentDownloadRow>(
        r#"
        SELECT files.storage_key, files.original_file_name, files.mime_type
        FROM asset_attachment_assignments AS assignments
        JOIN files ON files.file_id = assignments.file_id
        WHERE assignments.laboratory_id = $1
          AND assignments.attachment_id = $2
          AND assignments.is_public
        "#,
    )
    .bind(laboratory_id)
    .bind(attachment_id)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)
}
