use crate::authentication::{UserId, get_actor};
use crate::routes::{
    AssetListQuery, BorrowRequestListQuery, InventoryItemListQuery, MaintenanceRecordListQuery,
    fetch_assets, fetch_borrow_requests, fetch_inventory_items, fetch_maintenance_records,
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
            "is_cross_lab_borrowable",
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
                item.is_cross_lab_borrowable.to_string(),
                optional(item.public_notes),
                optional(item.internal_notes),
            ],
        );
    }
    Ok(csv_response("inventory-items.csv", csv))
}

#[tracing::instrument(name = "Export borrow requests CSV", skip(pool), fields(user_id=%user_id))]
pub async fn export_borrow_requests_csv(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<BorrowRequestListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let (requests, _) = fetch_borrow_requests(pool.get_ref(), &actor, &query, false).await?;

    let mut csv = String::new();
    push_csv_row(
        &mut csv,
        &[
            "borrow_request_id",
            "asset",
            "model",
            "requester",
            "requester_laboratory",
            "owner_laboratory",
            "requested_quantity",
            "unit",
            "expected_borrowed_at",
            "expected_returned_at",
            "purpose",
            "status",
            "review_comment",
            "created_at",
        ],
    );
    for request in requests {
        push_csv_row(
            &mut csv,
            &[
                request.borrow_request_id.to_string(),
                request.asset_name,
                optional(request.asset_model),
                request.requester_username,
                request.requester_laboratory_name,
                request.owner_laboratory_name,
                request.requested_quantity.to_string(),
                request.unit_code,
                optional_datetime(request.expected_borrowed_at),
                optional_datetime(request.expected_returned_at),
                request.purpose,
                request.status,
                optional(request.review_comment),
                request.created_at.to_rfc3339(),
            ],
        );
    }
    Ok(csv_response("borrow-requests.csv", csv))
}

#[tracing::instrument(name = "Export maintenance records CSV", skip(pool), fields(user_id=%user_id))]
pub async fn export_maintenance_records_csv(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<MaintenanceRecordListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let (records, _) = fetch_maintenance_records(pool.get_ref(), &actor, &query, false).await?;

    let mut csv = String::new();
    push_csv_row(
        &mut csv,
        &[
            "maintenance_record_id",
            "asset",
            "model",
            "laboratory",
            "maintenance_type",
            "maintained_at",
            "responsible_user",
            "description",
            "public_notes",
            "internal_notes",
            "created_at",
        ],
    );
    for record in records {
        push_csv_row(
            &mut csv,
            &[
                record.maintenance_record_id.to_string(),
                record.asset_name,
                optional(record.asset_model),
                record.laboratory_name,
                record.maintenance_type,
                record.maintained_at.to_rfc3339(),
                optional(record.responsible_username),
                record.description,
                optional(record.public_notes),
                optional(record.internal_notes),
                record.created_at.to_rfc3339(),
            ],
        );
    }
    Ok(csv_response("maintenance-records.csv", csv))
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

fn optional_datetime(value: Option<chrono::DateTime<chrono::Utc>>) -> String {
    value.map(|v| v.to_rfc3339()).unwrap_or_default()
}
