use super::helpers::asset_list_select;
use super::model::{AssetResponse, AssetRow};
use crate::authentication::{UserId, get_actor};
use crate::routes::{PaginatedResponse, Pagination, normalized_search_pattern};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AssetListQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub q: Option<String>,
    pub laboratory_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub cascade: Option<bool>,
    pub asset_kind: Option<String>,
    pub tracking_mode: Option<String>,
    pub is_archived: Option<bool>,
}

#[tracing::instrument(name = "List assets", skip(pool), fields(user_id=%user_id))]
pub async fn list_assets(
    user_id: UserId,
    pool: web::Data<PgPool>,
    query: web::Query<AssetListQuery>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let (assets, total) = fetch_assets(pool.get_ref(), &actor, &query, true).await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(assets, &query.pagination, total)?))
}

pub(crate) async fn fetch_assets(
    pool: &PgPool,
    actor: &crate::authentication::Actor,
    query: &AssetListQuery,
    paginate: bool,
) -> Result<(Vec<AssetResponse>, i64), ApiError> {
    let total = fetch_asset_count(pool, query).await?;
    let mut builder = QueryBuilder::<Postgres>::new(asset_list_select());
    push_asset_filters(&mut builder, query);
    builder.push(" ORDER BY laboratories.name, assets.name, assets.model");
    if paginate {
        builder.push(" LIMIT ");
        builder.push_bind(query.pagination.limit()?);
        builder.push(" OFFSET ");
        builder.push_bind(query.pagination.offset()?);
    }
    let assets = builder
        .build_query_as::<AssetRow>()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .into_iter()
        .map(|asset| AssetResponse::from_row(asset, actor))
        .collect::<Vec<_>>();
    Ok((assets, total))
}

async fn fetch_asset_count(pool: &PgPool, query: &AssetListQuery) -> Result<i64, ApiError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT COUNT(*)
        FROM assets
        INNER JOIN laboratories USING (laboratory_id)
        INNER JOIN units ON units.unit_id = assets.default_unit_id
        LEFT JOIN units AS minimum_stock_units ON minimum_stock_units.unit_id = assets.minimum_stock_unit_id
        LEFT JOIN asset_categories ON asset_categories.category_id = assets.category_id
        "#,
    );
    push_asset_filters(&mut builder, query);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))
}

fn push_asset_filters(builder: &mut QueryBuilder<'_, Postgres>, query: &AssetListQuery) {
    builder.push(" WHERE TRUE");
    if let Some(pattern) = normalized_search_pattern(&query.q) {
        builder.push(" AND (assets.name ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(assets.model, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(assets.manufacturer, '') ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    if let Some(laboratory_id) = query.laboratory_id {
        builder.push(" AND assets.laboratory_id = ");
        builder.push_bind(laboratory_id);
    }
    if let Some(category_id) = query.category_id {
        if query.cascade.unwrap_or(false) {
            builder.push(
                r#"
                AND assets.category_id IN (
                    WITH RECURSIVE descendants AS (
                        SELECT category_id
                        FROM asset_categories
                        WHERE category_id =
                "#,
            );
            builder.push_bind(category_id);
            builder.push(
                r#"
                        UNION ALL

                        SELECT child.category_id
                        FROM asset_categories child
                        INNER JOIN descendants ON descendants.category_id = child.parent_category_id
                    )
                    SELECT category_id
                    FROM descendants
                )
                "#,
            );
        } else {
            builder.push(" AND assets.category_id = ");
            builder.push_bind(category_id);
        }
    }
    if let Some(asset_kind) = query
        .asset_kind
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder.push(" AND assets.asset_kind = ");
        builder.push_bind(asset_kind.to_owned());
    }
    if let Some(tracking_mode) = query
        .tracking_mode
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder.push(" AND assets.tracking_mode = ");
        builder.push_bind(tracking_mode.to_owned());
    }
    if let Some(is_archived) = query.is_archived {
        builder.push(" AND assets.is_archived = ");
        builder.push_bind(is_archived);
    }
}
