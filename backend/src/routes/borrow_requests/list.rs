use super::helpers::borrow_request_select;
use super::model::BorrowRequest;
use crate::authentication::{Actor, UserId, get_actor};
use crate::routes::{PaginatedResponse, Pagination, normalized_search_pattern};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BorrowRequestListQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub q: Option<String>,
    pub laboratory_id: Option<Uuid>,
    pub status: Option<String>,
}

#[tracing::instrument(name = "List borrow requests", skip(pool), fields(user_id=%user_id))]
pub async fn list_borrow_requests(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<BorrowRequestListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let (borrow_requests, total) =
        fetch_borrow_requests(pool.get_ref(), &actor, &query, true).await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(
        borrow_requests,
        &query.pagination,
        total,
    )?))
}

pub(crate) async fn fetch_borrow_requests(
    pool: &PgPool,
    actor: &Actor,
    query: &BorrowRequestListQuery,
    paginate: bool,
) -> Result<(Vec<BorrowRequest>, i64), ApiError> {
    let total = fetch_borrow_request_count(pool, actor, query).await?;
    let mut builder = QueryBuilder::<Postgres>::new(borrow_request_select());
    push_borrow_request_filters(&mut builder, actor, query);
    builder.push(" ORDER BY borrow_requests.created_at DESC");
    if paginate {
        builder.push(" LIMIT ");
        builder.push_bind(query.pagination.limit()?);
        builder.push(" OFFSET ");
        builder.push_bind(query.pagination.offset()?);
    }
    let borrow_requests = builder
        .build_query_as::<BorrowRequest>()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    Ok((borrow_requests, total))
}

async fn fetch_borrow_request_count(
    pool: &PgPool,
    actor: &Actor,
    query: &BorrowRequestListQuery,
) -> Result<i64, ApiError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT COUNT(*)
        FROM borrow_requests
        INNER JOIN asset_inventory_items USING (inventory_item_id)
        INNER JOIN assets USING (asset_id)
        INNER JOIN users AS requester ON requester.user_id = borrow_requests.requester_user_id
        INNER JOIN laboratories AS requester_laboratory ON requester_laboratory.laboratory_id = borrow_requests.requester_laboratory_id
        INNER JOIN laboratories AS owner_laboratory ON owner_laboratory.laboratory_id = borrow_requests.owner_laboratory_id
        INNER JOIN units ON units.unit_id = borrow_requests.unit_id
        "#,
    );
    push_borrow_request_filters(&mut builder, actor, query);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))
}

fn push_borrow_request_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    actor: &Actor,
    query: &BorrowRequestListQuery,
) {
    builder.push(" WHERE TRUE");
    if !actor.is_system_admin() {
        if let Some(laboratory_id) = actor.laboratory_id {
            builder.push(" AND (borrow_requests.requester_laboratory_id = ");
            builder.push_bind(laboratory_id);
            builder.push(" OR borrow_requests.owner_laboratory_id = ");
            builder.push_bind(laboratory_id);
            builder.push(")");
        } else {
            builder.push(" AND FALSE");
        }
    }
    if let Some(pattern) = normalized_search_pattern(&query.q) {
        builder.push(" AND (assets.name ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(assets.model, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR borrow_requests.purpose ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR requester.username ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR requester_laboratory.name ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR owner_laboratory.name ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    if let Some(laboratory_id) = query.laboratory_id {
        builder.push(" AND (borrow_requests.requester_laboratory_id = ");
        builder.push_bind(laboratory_id);
        builder.push(" OR borrow_requests.owner_laboratory_id = ");
        builder.push_bind(laboratory_id);
        builder.push(")");
    }
    if let Some(status) = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder.push(" AND borrow_requests.status = ");
        builder.push_bind(status.to_owned());
    }
}
