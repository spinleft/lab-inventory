use super::helpers::maintenance_schedule_select;
use super::model::{MaintenanceScheduleResponse, MaintenanceScheduleRow};
use crate::authentication::{Actor, UserId, get_actor};
use crate::routes::{PaginatedResponse, Pagination, normalized_search_pattern};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MaintenanceScheduleListQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub q: Option<String>,
    pub laboratory_id: Option<Uuid>,
    pub asset_id: Option<Uuid>,
    pub inventory_item_id: Option<Uuid>,
    pub is_active: Option<bool>,
}

#[tracing::instrument(name = "List maintenance schedules", skip(pool), fields(user_id=%user_id))]
pub async fn list_maintenance_schedules(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<MaintenanceScheduleListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let (schedules, total) =
        fetch_maintenance_schedules(pool.get_ref(), &actor, &query, true).await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(schedules, &query.pagination, total)?))
}

pub(crate) async fn fetch_maintenance_schedules(
    pool: &PgPool,
    actor: &Actor,
    query: &MaintenanceScheduleListQuery,
    paginate: bool,
) -> Result<(Vec<MaintenanceScheduleResponse>, i64), ApiError> {
    let total = fetch_maintenance_schedule_count(pool, query).await?;
    let mut builder = QueryBuilder::<Postgres>::new(maintenance_schedule_select());
    push_maintenance_schedule_filters(&mut builder, query);
    builder.push(" ORDER BY maintenance_schedules.next_maintenance_at ASC, maintenance_schedules.created_at DESC");
    if paginate {
        builder.push(" LIMIT ");
        builder.push_bind(query.pagination.limit()?);
        builder.push(" OFFSET ");
        builder.push_bind(query.pagination.offset()?);
    }
    let schedules = builder
        .build_query_as::<MaintenanceScheduleRow>()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .into_iter()
        .map(|schedule| MaintenanceScheduleResponse::from_row(schedule, actor))
        .collect::<Vec<_>>();
    Ok((schedules, total))
}

async fn fetch_maintenance_schedule_count(
    pool: &PgPool,
    query: &MaintenanceScheduleListQuery,
) -> Result<i64, ApiError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT COUNT(*)
        FROM maintenance_schedules
        INNER JOIN laboratories USING (laboratory_id)
        LEFT JOIN assets AS schedule_asset ON schedule_asset.asset_id = maintenance_schedules.asset_id
        LEFT JOIN asset_inventory_items ON asset_inventory_items.inventory_item_id = maintenance_schedules.inventory_item_id
        LEFT JOIN assets AS item_asset ON item_asset.asset_id = asset_inventory_items.asset_id
        "#,
    );
    push_maintenance_schedule_filters(&mut builder, query);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))
}

fn push_maintenance_schedule_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    query: &MaintenanceScheduleListQuery,
) {
    builder.push(" WHERE TRUE");
    if let Some(pattern) = normalized_search_pattern(&query.q) {
        builder.push(" AND (COALESCE(schedule_asset.name, item_asset.name) ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(schedule_asset.model, item_asset.model, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR maintenance_schedules.schedule_name ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    if let Some(laboratory_id) = query.laboratory_id {
        builder.push(" AND maintenance_schedules.laboratory_id = ");
        builder.push_bind(laboratory_id);
    }
    if let Some(asset_id) = query.asset_id {
        builder.push(" AND maintenance_schedules.asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(inventory_item_id) = query.inventory_item_id {
        builder.push(" AND maintenance_schedules.inventory_item_id = ");
        builder.push_bind(inventory_item_id);
    }
    if let Some(is_active) = query.is_active {
        builder.push(" AND maintenance_schedules.is_active = ");
        builder.push_bind(is_active);
    }
}
