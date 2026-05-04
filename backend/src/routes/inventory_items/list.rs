use super::helpers::inventory_list_select;
use super::model::{InventoryItemResponse, InventoryItemRow};
use crate::authentication::{Actor, UserId, get_actor};
use crate::routes::{PaginatedResponse, Pagination, normalized_search_pattern};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct InventoryItemListQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub q: Option<String>,
    pub laboratory_id: Option<Uuid>,
    pub asset_id: Option<Uuid>,
    pub tracking_mode: Option<String>,
    pub status: Option<String>,
    pub is_cross_lab_borrowable: Option<bool>,
    pub location_id: Option<Uuid>,
}

#[tracing::instrument(name = "List inventory items", skip(pool), fields(user_id=%user_id))]
pub async fn list_inventory_items(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<InventoryItemListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let (items, total) = fetch_inventory_items(pool.get_ref(), &actor, &query, true).await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(items, &query.pagination, total)?))
}

pub(crate) async fn fetch_inventory_items(
    pool: &PgPool,
    actor: &Actor,
    query: &InventoryItemListQuery,
    paginate: bool,
) -> Result<(Vec<InventoryItemResponse>, i64), ApiError> {
    let total = fetch_inventory_item_count(pool, actor, query).await?;
    let mut builder = QueryBuilder::<Postgres>::new(inventory_list_select());
    push_inventory_filters(&mut builder, actor, query);
    builder.push(" ORDER BY laboratories.name, assets.name, asset_inventory_items.created_at");
    if paginate {
        builder.push(" LIMIT ");
        builder.push_bind(query.pagination.limit()?);
        builder.push(" OFFSET ");
        builder.push_bind(query.pagination.offset()?);
    }
    let items = builder
        .build_query_as::<InventoryItemRow>()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .into_iter()
        .map(|item| InventoryItemResponse::from_row(item, actor))
        .collect::<Vec<_>>();
    Ok((items, total))
}

async fn fetch_inventory_item_count(
    pool: &PgPool,
    actor: &Actor,
    query: &InventoryItemListQuery,
) -> Result<i64, ApiError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT COUNT(*)
        FROM asset_inventory_items
        INNER JOIN assets USING (asset_id)
        INNER JOIN laboratories ON laboratories.laboratory_id = asset_inventory_items.laboratory_id
        INNER JOIN units ON units.unit_id = asset_inventory_items.unit_id
        LEFT JOIN locations ON locations.location_id = asset_inventory_items.location_id
        "#,
    );
    push_inventory_filters(&mut builder, actor, query);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))
}

fn push_inventory_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    actor: &Actor,
    query: &InventoryItemListQuery,
) {
    builder.push(" WHERE TRUE");
    if let Some(pattern) = normalized_search_pattern(&query.q) {
        builder.push(" AND (assets.name ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(assets.model, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(asset_inventory_items.public_notes, '') ILIKE ");
        builder.push_bind(pattern.clone());
        if actor.is_system_admin() {
            builder.push(" OR COALESCE(asset_inventory_items.serial_number, '') ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR COALESCE(asset_inventory_items.batch_number, '') ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR COALESCE(locations.name, '') ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR COALESCE(asset_inventory_items.internal_notes, '') ILIKE ");
            builder.push_bind(pattern.clone());
        } else if let Some(laboratory_id) = actor.laboratory_id {
            builder.push(" OR (asset_inventory_items.laboratory_id = ");
            builder.push_bind(laboratory_id);
            builder.push(" AND (COALESCE(asset_inventory_items.serial_number, '') ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR COALESCE(asset_inventory_items.batch_number, '') ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR COALESCE(locations.name, '') ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR COALESCE(asset_inventory_items.internal_notes, '') ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push("))");
        }
        builder.push(")");
    }
    if let Some(laboratory_id) = query.laboratory_id {
        builder.push(" AND asset_inventory_items.laboratory_id = ");
        builder.push_bind(laboratory_id);
    }
    if let Some(asset_id) = query.asset_id {
        builder.push(" AND asset_inventory_items.asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(tracking_mode) = query
        .tracking_mode
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder.push(" AND asset_inventory_items.tracking_mode = ");
        builder.push_bind(tracking_mode.to_owned());
    }
    if let Some(status) = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder.push(" AND asset_inventory_items.status = ");
        builder.push_bind(status.to_owned());
    }
    if let Some(is_cross_lab_borrowable) = query.is_cross_lab_borrowable {
        builder.push(" AND asset_inventory_items.is_cross_lab_borrowable = ");
        builder.push_bind(is_cross_lab_borrowable);
    }
    if let Some(location_id) = query.location_id {
        builder.push(" AND asset_inventory_items.location_id = ");
        builder.push_bind(location_id);
    }
}
