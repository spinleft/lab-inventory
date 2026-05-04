use super::helpers::maintenance_record_select;
use super::model::{MaintenanceRecordResponse, MaintenanceRecordRow};
use crate::authentication::{Actor, UserId, get_actor};
use crate::routes::{PaginatedResponse, Pagination, normalized_search_pattern};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MaintenanceRecordListQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub q: Option<String>,
    pub laboratory_id: Option<Uuid>,
    pub asset_id: Option<Uuid>,
    pub inventory_item_id: Option<Uuid>,
    pub maintenance_type: Option<String>,
    pub maintained_from: Option<DateTime<Utc>>,
    pub maintained_to: Option<DateTime<Utc>>,
}

#[tracing::instrument(name = "List maintenance records", skip(pool), fields(user_id=%user_id))]
pub async fn list_maintenance_records(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<MaintenanceRecordListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let (records, total) = fetch_maintenance_records(pool.get_ref(), &actor, &query, true).await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(records, &query.pagination, total)?))
}

pub(crate) async fn fetch_maintenance_records(
    pool: &PgPool,
    actor: &Actor,
    query: &MaintenanceRecordListQuery,
    paginate: bool,
) -> Result<(Vec<MaintenanceRecordResponse>, i64), ApiError> {
    let total = fetch_maintenance_record_count(pool, query).await?;
    let mut builder = QueryBuilder::<Postgres>::new(maintenance_record_select());
    push_maintenance_record_filters(&mut builder, query);
    builder.push(
        " ORDER BY maintenance_records.maintained_at DESC, maintenance_records.created_at DESC",
    );
    if paginate {
        builder.push(" LIMIT ");
        builder.push_bind(query.pagination.limit()?);
        builder.push(" OFFSET ");
        builder.push_bind(query.pagination.offset()?);
    }
    let records = builder
        .build_query_as::<MaintenanceRecordRow>()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .into_iter()
        .map(|record| MaintenanceRecordResponse::from_row(record, actor))
        .collect::<Vec<_>>();
    Ok((records, total))
}

async fn fetch_maintenance_record_count(
    pool: &PgPool,
    query: &MaintenanceRecordListQuery,
) -> Result<i64, ApiError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT COUNT(*)
        FROM maintenance_records
        INNER JOIN laboratories USING (laboratory_id)
        LEFT JOIN assets AS record_asset ON record_asset.asset_id = maintenance_records.asset_id
        LEFT JOIN asset_inventory_items ON asset_inventory_items.inventory_item_id = maintenance_records.inventory_item_id
        LEFT JOIN assets AS item_asset ON item_asset.asset_id = asset_inventory_items.asset_id
        LEFT JOIN users AS responsible_user ON responsible_user.user_id = maintenance_records.responsible_user_id
        "#,
    );
    push_maintenance_record_filters(&mut builder, query);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))
}

fn push_maintenance_record_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    query: &MaintenanceRecordListQuery,
) {
    builder.push(" WHERE TRUE");
    if let Some(pattern) = normalized_search_pattern(&query.q) {
        builder.push(" AND (COALESCE(record_asset.name, item_asset.name) ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(record_asset.model, item_asset.model, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR maintenance_records.maintenance_type ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR maintenance_records.description ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    if let Some(laboratory_id) = query.laboratory_id {
        builder.push(" AND maintenance_records.laboratory_id = ");
        builder.push_bind(laboratory_id);
    }
    if let Some(asset_id) = query.asset_id {
        builder.push(" AND maintenance_records.asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(inventory_item_id) = query.inventory_item_id {
        builder.push(" AND maintenance_records.inventory_item_id = ");
        builder.push_bind(inventory_item_id);
    }
    if let Some(maintenance_type) = query
        .maintenance_type
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder.push(" AND maintenance_records.maintenance_type = ");
        builder.push_bind(maintenance_type.to_owned());
    }
    if let Some(maintained_from) = query.maintained_from {
        builder.push(" AND maintenance_records.maintained_at >= ");
        builder.push_bind(maintained_from);
    }
    if let Some(maintained_to) = query.maintained_to {
        builder.push(" AND maintenance_records.maintained_at <= ");
        builder.push_bind(maintained_to);
    }
}
