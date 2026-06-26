use super::model::{
    InventoryItemError, InventoryItemResponse, InventoryItemRow, actor_for_user,
    inventory_item_select, validate_read_permission, validate_status,
};
use crate::domain::{AssetTrackingMode, LaboratoryId, UserId};
use crate::routes::parameter_filters::{
    ParameterFilter, ParameterFilterError, parse_parameter_filters, push_parameter_filters,
};
use crate::routes::{PaginatedResponse, Pagination};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ListInventoryItemsQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub keyword: Option<String>,
    pub asset_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub exact_category: Option<bool>,
    pub tracking_mode: Option<String>,
    pub status: Option<String>,
    pub serial_number: Option<String>,
    pub batch_number: Option<String>,
    pub location_id: Option<Uuid>,
    pub has_batch: Option<bool>,
    pub has_location: Option<bool>,
    pub parameter_filters: Option<String>,
}

#[tracing::instrument(
    name = "List inventory items",
    skip(pool, query),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn list_inventory_items(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    query: web::Query<ListInventoryItemsQuery>,
) -> Result<HttpResponse, InventoryItemError> {
    validate_query(&query)?;
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let laboratory_id = validate_read_permission(&actor, laboratory_id.into_inner())?;
    let parameter_filters =
        parse_parameter_filters(&pool, laboratory_id, query.parameter_filters.as_deref())
            .await
            .map_err(map_parameter_filter_error)?;
    let include_internal_notes = actor.can_read_laboratory_resource(&laboratory_id);
    let total =
        fetch_inventory_item_count(&pool, laboratory_id, &query, &parameter_filters).await?;
    let limit = query.pagination.limit()?;
    let offset = query.pagination.offset()?;

    let mut builder = QueryBuilder::<Postgres>::new(inventory_item_select());
    push_inventory_item_filters(&mut builder, laboratory_id, &query, &parameter_filters);
    builder.push(
        " ORDER BY asset_inventory_items.updated_at DESC, asset_inventory_items.inventory_item_id",
    );
    builder.push(" LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    let rows = builder
        .build_query_as::<InventoryItemRow>()
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| InventoryItemError::UnexpectedError(e.into()))?;
    let items = rows
        .into_iter()
        .map(|row| InventoryItemResponse::from_row(row, include_internal_notes))
        .collect();
    Ok(HttpResponse::Ok().json(PaginatedResponse::new(items, &query.pagination, total)?))
}

fn validate_query(query: &ListInventoryItemsQuery) -> Result<(), InventoryItemError> {
    if let Some(tracking_mode) = query.tracking_mode.as_deref() {
        AssetTrackingMode::parse(tracking_mode).map_err(InventoryItemError::ValidationError)?;
    }
    validate_status(query.status.clone())?;
    Ok(())
}

async fn fetch_inventory_item_count(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    query: &ListInventoryItemsQuery,
    parameter_filters: &[ParameterFilter],
) -> Result<i64, InventoryItemError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT COUNT(*)
        FROM asset_inventory_items
        JOIN assets
          ON assets.asset_id = asset_inventory_items.asset_id
        "#,
    );
    push_inventory_item_filters(&mut builder, laboratory_id, query, parameter_filters);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| InventoryItemError::UnexpectedError(e.into()))
}

pub(super) fn push_inventory_item_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    laboratory_id: LaboratoryId,
    query: &ListInventoryItemsQuery,
    parameter_filters: &[ParameterFilter],
) {
    builder.push(" WHERE asset_inventory_items.laboratory_id = ");
    builder.push_bind(*laboratory_id);

    if let Some(keyword) = query
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|keyword| !keyword.is_empty())
    {
        let pattern = format!("%{keyword}%");
        builder.push(
            r#"
            AND (
                assets.name ILIKE
            "#,
        );
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(assets.model, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(assets.manufacturer, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(asset_inventory_items.serial_number, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(asset_inventory_items.batch_number, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(asset_inventory_items.public_notes, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(asset_inventory_items.internal_notes, '') ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }

    if let Some(asset_id) = query.asset_id {
        builder.push(" AND asset_inventory_items.asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(category_id) = query.category_id {
        if query.exact_category.unwrap_or(false) {
            builder.push(" AND assets.category_id = ");
            builder.push_bind(category_id);
        } else {
            builder.push(
                r#"
                AND assets.category_id IN (
                    SELECT child.category_id
                    FROM asset_categories AS root
                    JOIN asset_categories AS child
                      ON child.laboratory_id = root.laboratory_id
                     AND child.path <@ root.path
                    WHERE root.laboratory_id =
                "#,
            );
            builder.push_bind(*laboratory_id);
            builder.push(" AND root.category_id = ");
            builder.push_bind(category_id);
            builder.push(")");
        }
    }
    if let Some(tracking_mode) = query
        .tracking_mode
        .as_deref()
        .map(str::trim)
        .filter(|tracking_mode| !tracking_mode.is_empty())
    {
        builder.push(" AND asset_inventory_items.tracking_mode = ");
        builder.push_bind(tracking_mode.to_string());
    }
    if let Some(status) = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|status| !status.is_empty())
    {
        builder.push(" AND asset_inventory_items.status = ");
        builder.push_bind(status.to_string());
    }
    if let Some(serial_number) = query
        .serial_number
        .as_deref()
        .map(str::trim)
        .filter(|serial_number| !serial_number.is_empty())
    {
        builder.push(" AND asset_inventory_items.serial_number = ");
        builder.push_bind(serial_number.to_string());
    }
    if let Some(batch_number) = query
        .batch_number
        .as_deref()
        .map(str::trim)
        .filter(|batch_number| !batch_number.is_empty())
    {
        builder.push(" AND asset_inventory_items.batch_number = ");
        builder.push_bind(batch_number.to_string());
    }
    if let Some(location_id) = query.location_id {
        builder.push(" AND asset_inventory_items.location_id = ");
        builder.push_bind(location_id);
    }
    if let Some(has_batch) = query.has_batch {
        if has_batch {
            builder.push(" AND asset_inventory_items.batch_number IS NOT NULL");
        } else {
            builder.push(" AND asset_inventory_items.batch_number IS NULL");
        }
    }
    if let Some(has_location) = query.has_location {
        if has_location {
            builder.push(" AND asset_inventory_items.location_id IS NOT NULL");
        } else {
            builder.push(" AND asset_inventory_items.location_id IS NULL");
        }
    }
    push_parameter_filters(builder, "asset_inventory_items.asset_id", parameter_filters);
}

fn map_parameter_filter_error(error: ParameterFilterError) -> InventoryItemError {
    match error {
        ParameterFilterError::Validation(message) => InventoryItemError::ValidationError(message),
        ParameterFilterError::Unexpected(error) => InventoryItemError::UnexpectedError(error),
    }
}
