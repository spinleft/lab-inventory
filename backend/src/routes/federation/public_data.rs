use super::model::FederationError;
use crate::attachment_storage::AttachmentStorage;
use crate::domain::AttachmentStorageKey;
use actix_web::HttpResponse;
use actix_web::http::header;
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::collections::HashMap;
use url::form_urlencoded;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(super) enum FederationReadTarget {
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
struct PaginatedJson<T> {
    items: Vec<T>,
    limit: i64,
    offset: i64,
    total: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct LaboratoryPublicRow {
    laboratory_id: Uuid,
    name: String,
    address: String,
    description: Option<String>,
    contact: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct AssetPublicRow {
    asset_id: Uuid,
    laboratory_id: Uuid,
    category_id: Option<Uuid>,
    tracking_mode: String,
    name: String,
    model: Option<String>,
    manufacturer: Option<String>,
    default_unit_id: Uuid,
    public_notes: Option<String>,
    inventory_item_count: i64,
    quantity_on_hand: f64,
    quantity_allocated: f64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct AssetPublicResponse {
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
    inventory_items: Option<Vec<InventoryItemPublicResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Vec<ParameterValueResponse>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct AssetInventorySummary {
    item_count: i64,
    quantity_on_hand: f64,
    quantity_allocated: f64,
}

impl AssetPublicResponse {
    fn from_row(
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
            default_unit_id: row.default_unit_id,
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
struct InventoryItemPublicRow {
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
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_stocktake_at: Option<DateTime<Utc>>,
    asset_category_id: Option<Uuid>,
    asset_name: String,
    asset_model: Option<String>,
    asset_manufacturer: Option<String>,
    asset_default_unit_id: Uuid,
}

#[derive(Serialize)]
struct InventoryItemPublicResponse {
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
    asset: InventoryItemAssetResponse,
}

#[derive(Serialize)]
struct InventoryItemAssetResponse {
    asset_id: Uuid,
    category_id: Option<Uuid>,
    name: String,
    model: Option<String>,
    manufacturer: Option<String>,
    default_unit_id: Uuid,
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
            quantity_unit_id: row.quantity_unit_id,
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
                default_unit_id: row.asset_default_unit_id,
            },
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
struct CategoryRow {
    category_id: Uuid,
    laboratory_id: Uuid,
    parent_category_id: Option<Uuid>,
    name: String,
    code: String,
    path: String,
    depth: i32,
    description: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct LocationRow {
    location_id: Uuid,
    laboratory_id: Uuid,
    parent_location_id: Option<Uuid>,
    name: String,
    code: String,
    path: String,
    depth: i32,
    description: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct ParameterRow {
    parameter_type_id: Uuid,
    laboratory_id: Uuid,
    code: String,
    name: String,
    data_type: String,
    unit_dimension: Option<String>,
    default_unit_id: Option<Uuid>,
    description: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct ParameterOptionRow {
    option_id: Uuid,
    parameter_type_id: Uuid,
    code: String,
    label: String,
    sort_order: i32,
}

#[derive(Serialize)]
struct ParameterResponse {
    parameter_type_id: Uuid,
    laboratory_id: Uuid,
    code: String,
    name: String,
    data_type: String,
    unit_dimension: Option<String>,
    default_unit_id: Option<Uuid>,
    description: Option<String>,
    options: Vec<ParameterOptionRow>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct ParameterValueRow {
    value_id: Uuid,
    laboratory_id: Uuid,
    asset_id: Uuid,
    parameter_type_id: Uuid,
    code: String,
    name: String,
    data_type: String,
    unit_dimension: Option<String>,
    default_unit_id: Option<Uuid>,
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
    option_code: Option<String>,
    option_label: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct ParameterValueResponse {
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

impl From<ParameterValueRow> for ParameterValueResponse {
    fn from(row: ParameterValueRow) -> Self {
        let value = match row.data_type.as_str() {
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
struct AttachmentPublicRow {
    attachment_id: Uuid,
    laboratory_id: Uuid,
    asset_id: Option<Uuid>,
    inventory_item_id: Option<Uuid>,
    display_name: String,
    original_file_name: String,
    description: Option<String>,
    mime_type: Option<String>,
    file_size_bytes: i64,
    sha256_hex: String,
    visibility: String,
    uploaded_by_user_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct AttachmentDownloadRow {
    storage_key: String,
    original_file_name: String,
    mime_type: Option<String>,
}

pub(super) fn parse_read_target(tail: &str) -> Result<FederationReadTarget, FederationError> {
    let mut parts = tail
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty());
    let first = parts.next();
    let second = parts.next();
    let third = parts.next();
    if parts.next().is_some() {
        return Err(FederationError::NotFound(
            "Federation route not found".into(),
        ));
    }
    match (first, second, third) {
        (None, None, None) => Ok(FederationReadTarget::Laboratory),
        (Some("assets"), None, None) => Ok(FederationReadTarget::Assets),
        (Some("assets"), Some(asset_id), None) => {
            Ok(FederationReadTarget::Asset(parse_uuid(asset_id)?))
        }
        (Some("assets"), Some(asset_id), Some("attachments")) => Ok(
            FederationReadTarget::AssetAttachments(parse_uuid(asset_id)?),
        ),
        (Some("inventory-items"), None, None) => Ok(FederationReadTarget::InventoryItems),
        (Some("inventory-items"), Some(item_id), None) => {
            Ok(FederationReadTarget::InventoryItem(parse_uuid(item_id)?))
        }
        (Some("inventory-items"), Some(item_id), Some("attachments")) => Ok(
            FederationReadTarget::InventoryItemAttachments(parse_uuid(item_id)?),
        ),
        (Some("asset-categories"), None, None) => Ok(FederationReadTarget::AssetCategories),
        (Some("asset-categories"), Some(category_id), None) => Ok(
            FederationReadTarget::AssetCategory(parse_uuid(category_id)?),
        ),
        (Some("asset-parameters"), None, None) => Ok(FederationReadTarget::AssetParameters),
        (Some("asset-parameters"), Some(parameter_id), None) => Ok(
            FederationReadTarget::AssetParameter(parse_uuid(parameter_id)?),
        ),
        (Some("locations"), None, None) => Ok(FederationReadTarget::Locations),
        (Some("locations"), Some(location_id), None) => {
            Ok(FederationReadTarget::Location(parse_uuid(location_id)?))
        }
        (Some("attachments"), None, None) => Ok(FederationReadTarget::Attachments),
        (Some("attachments"), Some(attachment_id), None) => {
            Ok(FederationReadTarget::Attachment(parse_uuid(attachment_id)?))
        }
        (Some("attachments"), Some(attachment_id), Some("download")) => Ok(
            FederationReadTarget::AttachmentDownload(parse_uuid(attachment_id)?),
        ),
        _ => Err(FederationError::NotFound(
            "Federation route not found".into(),
        )),
    }
}

pub(super) async fn respond_public_data(
    pool: &PgPool,
    storage: &AttachmentStorage,
    laboratory_id: Uuid,
    target: FederationReadTarget,
    query_string: &str,
) -> Result<HttpResponse, FederationError> {
    match target {
        FederationReadTarget::Laboratory => {
            Ok(HttpResponse::Ok().json(fetch_laboratory(pool, laboratory_id).await?))
        }
        FederationReadTarget::Assets => {
            Ok(HttpResponse::Ok().json(list_assets(pool, laboratory_id, query_string).await?))
        }
        FederationReadTarget::Asset(asset_id) => Ok(HttpResponse::Ok()
            .json(fetch_asset_response(pool, laboratory_id, asset_id, query_string).await?)),
        FederationReadTarget::InventoryItems => {
            Ok(HttpResponse::Ok()
                .json(list_inventory_items(pool, laboratory_id, query_string).await?))
        }
        FederationReadTarget::InventoryItem(item_id) => Ok(HttpResponse::Ok()
            .json(fetch_inventory_item_response(pool, laboratory_id, item_id).await?)),
        FederationReadTarget::AssetCategories => {
            Ok(HttpResponse::Ok().json(list_categories(pool, laboratory_id, query_string).await?))
        }
        FederationReadTarget::AssetCategory(category_id) => {
            Ok(HttpResponse::Ok().json(fetch_category(pool, laboratory_id, category_id).await?))
        }
        FederationReadTarget::Locations => {
            Ok(HttpResponse::Ok().json(list_locations(pool, laboratory_id, query_string).await?))
        }
        FederationReadTarget::Location(location_id) => {
            Ok(HttpResponse::Ok().json(fetch_location(pool, laboratory_id, location_id).await?))
        }
        FederationReadTarget::AssetParameters => {
            Ok(HttpResponse::Ok().json(list_parameters(pool, laboratory_id).await?))
        }
        FederationReadTarget::AssetParameter(parameter_id) => Ok(HttpResponse::Ok()
            .json(fetch_parameter_response(pool, laboratory_id, parameter_id).await?)),
        FederationReadTarget::Attachments => Ok(HttpResponse::Ok()
            .json(list_laboratory_attachments(pool, laboratory_id, query_string).await?)),
        FederationReadTarget::Attachment(attachment_id) => Ok(
            HttpResponse::Ok().json(fetch_attachment(pool, laboratory_id, attachment_id).await?)
        ),
        FederationReadTarget::AssetAttachments(asset_id) => {
            Ok(HttpResponse::Ok()
                .json(list_asset_attachments(pool, laboratory_id, asset_id).await?))
        }
        FederationReadTarget::InventoryItemAttachments(item_id) => Ok(HttpResponse::Ok()
            .json(list_inventory_item_attachments(pool, laboratory_id, item_id).await?)),
        FederationReadTarget::AttachmentDownload(attachment_id) => {
            download_attachment(pool, storage, laboratory_id, attachment_id).await
        }
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, FederationError> {
    value
        .parse()
        .map_err(|_| FederationError::NotFound("Federation route not found".into()))
}

fn query_params(query_string: &str) -> HashMap<String, String> {
    form_urlencoded::parse(query_string.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

fn limit_offset(query_string: &str) -> Result<(i64, i64), FederationError> {
    let params = query_params(query_string);
    let limit = params
        .get("limit")
        .map(|value| value.parse::<i64>())
        .transpose()
        .map_err(|_| FederationError::ValidationError("limit must be a number".into()))?
        .unwrap_or(50);
    let offset = params
        .get("offset")
        .map(|value| value.parse::<i64>())
        .transpose()
        .map_err(|_| FederationError::ValidationError("offset must be a number".into()))?
        .unwrap_or(0);
    if limit <= 0 {
        return Err(FederationError::ValidationError(
            "limit must be positive".into(),
        ));
    }
    if offset < 0 {
        return Err(FederationError::ValidationError(
            "offset must be non-negative".into(),
        ));
    }
    Ok((limit.min(200), offset))
}

async fn fetch_laboratory(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<LaboratoryPublicRow, FederationError> {
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Laboratory not found".into()))
}

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
        assets.default_unit_id,
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

async fn list_assets(
    pool: &PgPool,
    laboratory_id: Uuid,
    query_string: &str,
) -> Result<PaginatedJson<AssetPublicResponse>, FederationError> {
    let (limit, offset) = limit_offset(query_string)?;
    let params = query_params(query_string);
    let total = fetch_asset_count(pool, laboratory_id, &params).await?;
    let mut builder = QueryBuilder::<Postgres>::new(asset_select());
    push_asset_filters(&mut builder, laboratory_id, &params);
    builder.push(" ORDER BY assets.updated_at DESC, assets.asset_id LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);
    let rows = builder
        .build_query_as::<AssetPublicRow>()
        .fetch_all(pool)
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let items = rows
        .into_iter()
        .map(|row| AssetPublicResponse::from_row(row, None, None))
        .collect();
    Ok(PaginatedJson {
        items,
        limit,
        offset,
        total,
    })
}

async fn fetch_asset_count(
    pool: &PgPool,
    laboratory_id: Uuid,
    params: &HashMap<String, String>,
) -> Result<i64, FederationError> {
    let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM assets");
    push_asset_filters(&mut builder, laboratory_id, params);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))
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

async fn fetch_asset_response(
    pool: &PgPool,
    laboratory_id: Uuid,
    asset_id: Uuid,
    query_string: &str,
) -> Result<AssetPublicResponse, FederationError> {
    let row = sqlx::query_as::<_, AssetPublicRow>(&format!(
        "{} WHERE assets.laboratory_id = $1 AND assets.asset_id = $2",
        asset_select()
    ))
    .bind(laboratory_id)
    .bind(asset_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Asset not found".into()))?;
    let inventory_items = fetch_inventory_items_for_asset(pool, laboratory_id, asset_id).await?;
    let include_parameters = query_params(query_string)
        .get("include")
        .is_some_and(|value| value.split(',').any(|part| part.trim() == "parameters"));
    let parameters = if include_parameters {
        Some(
            fetch_parameter_values(pool, &[asset_id])
                .await?
                .remove(&asset_id)
                .unwrap_or_default(),
        )
    } else {
        None
    };
    Ok(AssetPublicResponse::from_row(
        row,
        Some(inventory_items),
        parameters,
    ))
}

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
        asset_inventory_items.quantity_unit_id,
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
        assets.default_unit_id AS asset_default_unit_id
    FROM asset_inventory_items
    JOIN assets ON assets.asset_id = asset_inventory_items.asset_id
    "#
}

async fn list_inventory_items(
    pool: &PgPool,
    laboratory_id: Uuid,
    query_string: &str,
) -> Result<PaginatedJson<InventoryItemPublicResponse>, FederationError> {
    let (limit, offset) = limit_offset(query_string)?;
    let params = query_params(query_string);
    let total = fetch_inventory_item_count(pool, laboratory_id, &params).await?;
    let mut builder = QueryBuilder::<Postgres>::new(inventory_item_select());
    push_inventory_filters(&mut builder, laboratory_id, &params);
    builder.push(" ORDER BY asset_inventory_items.updated_at DESC, asset_inventory_items.inventory_item_id LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);
    let rows = builder
        .build_query_as::<InventoryItemPublicRow>()
        .fetch_all(pool)
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    Ok(PaginatedJson {
        items: rows
            .into_iter()
            .map(InventoryItemPublicResponse::from)
            .collect(),
        limit,
        offset,
        total,
    })
}

async fn fetch_inventory_item_count(
    pool: &PgPool,
    laboratory_id: Uuid,
    params: &HashMap<String, String>,
) -> Result<i64, FederationError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*) FROM asset_inventory_items JOIN assets ON assets.asset_id = asset_inventory_items.asset_id",
    );
    push_inventory_filters(&mut builder, laboratory_id, params);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))
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

async fn fetch_inventory_item_response(
    pool: &PgPool,
    laboratory_id: Uuid,
    inventory_item_id: Uuid,
) -> Result<InventoryItemPublicResponse, FederationError> {
    let row = sqlx::query_as::<_, InventoryItemPublicRow>(&format!(
        "{} WHERE asset_inventory_items.laboratory_id = $1 AND asset_inventory_items.inventory_item_id = $2",
        inventory_item_select()
    ))
    .bind(laboratory_id)
    .bind(inventory_item_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Inventory item not found".into()))?;
    Ok(InventoryItemPublicResponse::from(row))
}

async fn fetch_inventory_items_for_asset(
    pool: &PgPool,
    laboratory_id: Uuid,
    asset_id: Uuid,
) -> Result<Vec<InventoryItemPublicResponse>, FederationError> {
    let rows = sqlx::query_as::<_, InventoryItemPublicRow>(&format!(
        "{} WHERE asset_inventory_items.laboratory_id = $1 AND asset_inventory_items.asset_id = $2 ORDER BY asset_inventory_items.created_at, asset_inventory_items.inventory_item_id",
        inventory_item_select()
    ))
    .bind(laboratory_id)
    .bind(asset_id)
    .fetch_all(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    Ok(rows
        .into_iter()
        .map(InventoryItemPublicResponse::from)
        .collect())
}

async fn list_categories(
    pool: &PgPool,
    laboratory_id: Uuid,
    query_string: &str,
) -> Result<Vec<CategoryRow>, FederationError> {
    let params = query_params(query_string);
    let root_path = if let Some(root_id) = params
        .get("root_category_id")
        .and_then(|value| value.parse::<Uuid>().ok())
    {
        Some(fetch_category(pool, laboratory_id, root_id).await?.path)
    } else {
        None
    };
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))
}

async fn fetch_category(
    pool: &PgPool,
    laboratory_id: Uuid,
    category_id: Uuid,
) -> Result<CategoryRow, FederationError> {
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Asset category not found".into()))
}

async fn list_locations(
    pool: &PgPool,
    laboratory_id: Uuid,
    query_string: &str,
) -> Result<Vec<LocationRow>, FederationError> {
    let params = query_params(query_string);
    let root_path = if let Some(root_id) = params
        .get("root_location_id")
        .and_then(|value| value.parse::<Uuid>().ok())
    {
        Some(fetch_location(pool, laboratory_id, root_id).await?.path)
    } else {
        None
    };
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))
}

async fn fetch_location(
    pool: &PgPool,
    laboratory_id: Uuid,
    location_id: Uuid,
) -> Result<LocationRow, FederationError> {
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Location not found".into()))
}

async fn list_parameters(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Vec<ParameterResponse>, FederationError> {
    let rows = sqlx::query_as::<_, ParameterRow>(
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let mut response = Vec::with_capacity(rows.len());
    for row in rows {
        response.push(parameter_response(pool, row).await?);
    }
    Ok(response)
}

async fn fetch_parameter_response(
    pool: &PgPool,
    laboratory_id: Uuid,
    parameter_id: Uuid,
) -> Result<ParameterResponse, FederationError> {
    let row = sqlx::query_as::<_, ParameterRow>(
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Asset parameter not found".into()))?;
    parameter_response(pool, row).await
}

async fn parameter_response(
    pool: &PgPool,
    row: ParameterRow,
) -> Result<ParameterResponse, FederationError> {
    let options = sqlx::query_as::<_, ParameterOptionRow>(
        r#"
        SELECT option_id, parameter_type_id, code, label, sort_order
        FROM asset_parameter_options
        WHERE parameter_type_id = $1
        ORDER BY sort_order, label, code
        "#,
    )
    .bind(row.parameter_type_id)
    .fetch_all(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    Ok(ParameterResponse {
        parameter_type_id: row.parameter_type_id,
        laboratory_id: row.laboratory_id,
        code: row.code,
        name: row.name,
        data_type: row.data_type,
        unit_dimension: row.unit_dimension,
        default_unit_id: row.default_unit_id,
        description: row.description,
        options,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn fetch_parameter_values(
    pool: &PgPool,
    asset_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<ParameterValueResponse>>, FederationError> {
    if asset_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, ParameterValueRow>(
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
            asset_parameter_values.value_number_base,
            asset_parameter_values.value_range_start,
            asset_parameter_values.value_range_end,
            asset_parameter_values.value_range_start_base,
            asset_parameter_values.value_range_end_base,
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let mut values = HashMap::new();
    for row in rows {
        values
            .entry(row.asset_id)
            .or_insert_with(Vec::new)
            .push(ParameterValueResponse::from(row));
    }
    Ok(values)
}

async fn list_laboratory_attachments(
    pool: &PgPool,
    laboratory_id: Uuid,
    query_string: &str,
) -> Result<PaginatedJson<AttachmentPublicRow>, FederationError> {
    let (limit, offset) = limit_offset(query_string)?;
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM attachments
        WHERE laboratory_id = $1
          AND deleted_at IS NULL
          AND visibility = 'public'
        "#,
    )
    .bind(laboratory_id)
    .fetch_one(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let items = sqlx::query_as::<_, AttachmentPublicRow>(&attachment_select(
        "WHERE laboratory_id = $1 AND deleted_at IS NULL AND visibility = 'public' ORDER BY created_at DESC, attachment_id LIMIT $2 OFFSET $3",
    ))
    .bind(laboratory_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    Ok(PaginatedJson {
        items,
        limit,
        offset,
        total,
    })
}

async fn list_asset_attachments(
    pool: &PgPool,
    laboratory_id: Uuid,
    asset_id: Uuid,
) -> Result<Vec<AttachmentPublicRow>, FederationError> {
    sqlx::query_as::<_, AttachmentPublicRow>(&attachment_select(
        "WHERE laboratory_id = $1 AND asset_id = $2 AND deleted_at IS NULL AND visibility = 'public' ORDER BY created_at DESC, attachment_id",
    ))
    .bind(laboratory_id)
    .bind(asset_id)
    .fetch_all(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))
}

async fn list_inventory_item_attachments(
    pool: &PgPool,
    laboratory_id: Uuid,
    inventory_item_id: Uuid,
) -> Result<Vec<AttachmentPublicRow>, FederationError> {
    sqlx::query_as::<_, AttachmentPublicRow>(&attachment_select(
        "WHERE laboratory_id = $1 AND inventory_item_id = $2 AND deleted_at IS NULL AND visibility = 'public' ORDER BY created_at DESC, attachment_id",
    ))
    .bind(laboratory_id)
    .bind(inventory_item_id)
    .fetch_all(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))
}

async fn fetch_attachment(
    pool: &PgPool,
    laboratory_id: Uuid,
    attachment_id: Uuid,
) -> Result<AttachmentPublicRow, FederationError> {
    sqlx::query_as::<_, AttachmentPublicRow>(&attachment_select(
        "WHERE laboratory_id = $1 AND attachment_id = $2 AND deleted_at IS NULL AND visibility = 'public'",
    ))
    .bind(laboratory_id)
    .bind(attachment_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Attachment not found".into()))
}

async fn download_attachment(
    pool: &PgPool,
    storage: &AttachmentStorage,
    laboratory_id: Uuid,
    attachment_id: Uuid,
) -> Result<HttpResponse, FederationError> {
    let row = sqlx::query_as::<_, AttachmentDownloadRow>(
        r#"
        SELECT storage_key, original_file_name, mime_type
        FROM attachments
        WHERE laboratory_id = $1
          AND attachment_id = $2
          AND deleted_at IS NULL
          AND visibility = 'public'
        "#,
    )
    .bind(laboratory_id)
    .bind(attachment_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Attachment not found".into()))?;
    let storage_key = AttachmentStorageKey::parse(row.storage_key)
        .map_err(|e| FederationError::UnexpectedError(anyhow::anyhow!("{e}")))?;
    let bytes = storage
        .read(&storage_key)
        .await
        .map_err(FederationError::UnexpectedError)?;
    Ok(HttpResponse::Ok()
        .insert_header((
            header::CONTENT_TYPE,
            row.mime_type
                .unwrap_or_else(|| "application/octet-stream".to_string()),
        ))
        .insert_header((header::CONTENT_LENGTH, bytes.len().to_string()))
        .insert_header((
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                content_disposition_filename(&row.original_file_name)
            ),
        ))
        .body(bytes))
}

fn attachment_select(suffix: &str) -> String {
    format!(
        r#"
        SELECT
            attachment_id,
            laboratory_id,
            asset_id,
            inventory_item_id,
            display_name,
            original_file_name,
            description,
            mime_type,
            file_size_bytes,
            sha256_hex,
            visibility,
            uploaded_by_user_id,
            created_at,
            updated_at
        FROM attachments
        {suffix}
        "#
    )
}

fn content_disposition_filename(file_name: &str) -> String {
    file_name
        .chars()
        .map(|ch| match ch {
            '"' | '\\' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect()
}
