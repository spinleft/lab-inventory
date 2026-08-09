use crate::authentication::{UserId, get_actor};
use crate::routes::{
    AssetListQuery, InventoryItemListQuery,
    fetch_assets, fetch_inventory_items,
};
use crate::utils::ApiError;
use actix_web::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

#[tracing::instrument(name = "Export assets CSV", skip(pool), fields(user_id=%user_id))]
pub async fn export_assets_csv(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<AssetListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let (assets, _) = fetch_assets(pool.get_ref(), &actor, &query, false).await?;

    let mut csv = String::new();
    push_csv_row(
        &mut csv,
        &[
            "asset_id",
            "laboratory",
            "category",
            "asset_kind",
            "tracking_mode",
            "name",
            "model",
            "manufacturer",
            "default_unit",
            "minimum_stock_quantity",
            "minimum_stock_unit",
            "is_archived",
            "public_notes",
            "internal_notes",
        ],
    );
    for asset in assets {
        push_csv_row(
            &mut csv,
            &[
                asset.asset_id.to_string(),
                asset.laboratory_name,
                optional(asset.category_name),
                asset.asset_kind,
                asset.tracking_mode,
                asset.name,
                optional(asset.model),
                optional(asset.manufacturer),
                asset.default_unit_code,
                optional_number(asset.minimum_stock_quantity),
                optional(asset.minimum_stock_unit_code),
                asset.is_archived.to_string(),
                optional(asset.public_notes),
                optional(asset.internal_notes),
            ],
        );
    }
    Ok(csv_response("assets.csv", csv))
}

#[tracing::instrument(name = "Export inventory items CSV", skip(pool), fields(user_id=%user_id))]
pub async fn export_inventory_items_csv(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<InventoryItemListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let (items, _) = fetch_inventory_items(pool.get_ref(), &actor, &query, false).await?;

    let mut csv = String::new();
    push_csv_row(
        &mut csv,
        &[
            "inventory_item_id",
            "asset",
            "model",
            "laboratory",
            "tracking_mode",
            "serial_number",
            "batch_number",
            "quantity_on_hand",
            "quantity_allocated",
            "quantity_available",
            "unit",
            "location",
            "status",
            "public_notes",
            "internal_notes",
        ],
    );
    for item in items {
        push_csv_row(
            &mut csv,
            &[
                item.inventory_item_id.to_string(),
                item.asset_name,
                optional(item.asset_model),
                item.laboratory_name,
                item.tracking_mode,
                optional(item.serial_number),
                optional(item.batch_number),
                item.quantity_on_hand.to_string(),
                item.quantity_allocated.to_string(),
                item.quantity_available.to_string(),
                item.unit_code,
                optional(item.location_name),
                item.status,
                optional(item.public_notes),
                optional(item.internal_notes),
            ],
        );
    }
    Ok(csv_response("inventory-items.csv", csv))
}

fn csv_response(filename: &'static str, csv: String) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((CONTENT_TYPE, "text/csv; charset=utf-8"))
        .insert_header((
            CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        ))
        .body(csv)
}

fn push_csv_row<T: AsRef<str>>(csv: &mut String, fields: &[T]) {
    let line = fields
        .iter()
        .map(|field| escape_csv_field(field.as_ref()))
        .collect::<Vec<_>>()
        .join(",");
    csv.push_str(&line);
    csv.push('\n');
}

fn escape_csv_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn optional(value: Option<String>) -> String {
    value.unwrap_or_default()
}

fn optional_number(value: Option<f64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}
