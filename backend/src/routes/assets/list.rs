use super::model::{AssetResponse, AssetRow, asset_select, fetch_parameter_values_for_assets};
use crate::access_control::{Actor, get_actor};
use crate::domain::{AssetInventoryStatus, AssetTrackingMode, LaboratoryId, UserId};
use crate::routes::parameter_filters::{
    ParameterFilter, ParameterFilterError, parse_parameter_filters, push_parameter_filters,
};
use crate::routes::{PaginatedResponse, Pagination, PaginationError};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::anyhow;
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ListAssetsQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub keyword: Option<String>,
    pub category_id: Option<Uuid>,
    pub exact_category: Option<bool>,
    pub tracking_mode: Option<String>,
    pub manufacturer: Option<String>,
    pub inventory_status: Option<String>,
    pub location_id: Option<Uuid>,
    pub has_inventory: Option<bool>,
    pub include: Option<String>,
    pub parameter_filters: Option<String>,
}

#[derive(thiserror::Error)]
pub enum ListAssetsError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListAssetsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListAssetsError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListAssetsError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ListAssetsError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListAssetsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<PaginationError> for ListAssetsError {
    fn from(error: PaginationError) -> Self {
        Self::ValidationError(error.to_string())
    }
}

#[tracing::instrument(
    name = "List assets",
    skip(pool, query),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn list_assets(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    query: web::Query<ListAssetsQuery>,
) -> Result<HttpResponse, ListAssetsError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id.into_inner())
        .map_err(|e| ListAssetsError::UnexpectedError(anyhow!("{e}")))?;
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(ListAssetsError::UnexpectedError)?
        .ok_or(ListAssetsError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_read_permission(&actor, &laboratory_id)?;
    validate_query(&query)?;
    let parameter_filters =
        parse_parameter_filters(&pool, laboratory_id, query.parameter_filters.as_deref())
            .await
            .map_err(map_parameter_filter_error)?;
    let include_internal_notes = actor.can_read_laboratory_resource(&laboratory_id);

    let total = fetch_asset_count(&pool, laboratory_id, &query, &parameter_filters).await?;
    let limit = query.pagination.limit()?;
    let offset = query.pagination.offset()?;

    let mut builder = QueryBuilder::<Postgres>::new(asset_select());
    push_asset_filters(&mut builder, laboratory_id, &query, &parameter_filters);
    builder.push(" ORDER BY assets.updated_at DESC, assets.asset_id");
    builder.push(" LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    let assets = builder
        .build_query_as::<AssetRow>()
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| ListAssetsError::UnexpectedError(e.into()))?;

    let include_parameters = include_parameters(&query)?;
    let mut parameters_by_asset_id = if include_parameters {
        let asset_ids: Vec<_> = assets.iter().map(|asset| asset.asset_id).collect();
        fetch_parameter_values_for_assets(&pool, &asset_ids).await?
    } else {
        Default::default()
    };

    let items = assets
        .into_iter()
        .map(|asset| {
            let parameters = if include_parameters {
                Some(
                    parameters_by_asset_id
                        .remove(&asset.asset_id)
                        .unwrap_or_default(),
                )
            } else {
                None
            };
            AssetResponse::from_parts_with_internal_notes(
                asset,
                None,
                parameters,
                include_internal_notes,
            )
        })
        .collect();

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(items, &query.pagination, total)?))
}

fn validate_read_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), ListAssetsError> {
    if actor.can_query_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(ListAssetsError::Forbidden(
            "You do not have permission to view assets for this laboratory".into(),
        ))
    }
}

fn validate_query(query: &ListAssetsQuery) -> Result<(), ListAssetsError> {
    if let Some(tracking_mode) = query.tracking_mode.as_deref() {
        AssetTrackingMode::parse(tracking_mode).map_err(ListAssetsError::ValidationError)?;
    }
    if let Some(status) = query.inventory_status.as_deref() {
        AssetInventoryStatus::parse(status).map_err(ListAssetsError::ValidationError)?;
    }
    include_parameters(query)?;
    Ok(())
}

async fn fetch_asset_count(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    query: &ListAssetsQuery,
    parameter_filters: &[ParameterFilter],
) -> Result<i64, ListAssetsError> {
    let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM assets");
    push_asset_filters(&mut builder, laboratory_id, query, parameter_filters);
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| ListAssetsError::UnexpectedError(e.into()))
}

fn push_asset_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    laboratory_id: LaboratoryId,
    query: &ListAssetsQuery,
    parameter_filters: &[ParameterFilter],
) {
    builder.push(" WHERE assets.laboratory_id = ");
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
        builder.push(" OR COALESCE(assets.public_notes, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(
            r#"
                OR EXISTS (
                    SELECT 1
                    FROM asset_inventory_items AS inventory_items
                    WHERE inventory_items.asset_id = assets.asset_id
                      AND (
                          COALESCE(inventory_items.serial_number, '') ILIKE
            "#,
        );
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(inventory_items.batch_number, '') ILIKE ");
        builder.push_bind(pattern);
        builder.push("))");
        builder.push(")");
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
        builder.push(" AND assets.tracking_mode = ");
        builder.push_bind(tracking_mode.to_string());
    }
    if let Some(manufacturer) = query
        .manufacturer
        .as_deref()
        .map(str::trim)
        .filter(|manufacturer| !manufacturer.is_empty())
    {
        builder.push(" AND assets.manufacturer = ");
        builder.push_bind(manufacturer.to_string());
    }
    if let Some(status) = query.inventory_status.as_deref() {
        builder.push(
            r#"
            AND EXISTS (
                SELECT 1
                FROM asset_inventory_items AS inventory_items
                WHERE inventory_items.asset_id = assets.asset_id
                  AND inventory_items.status =
            "#,
        );
        builder.push_bind(status.trim().to_string());
        builder.push(")");
    }
    if let Some(location_id) = query.location_id {
        builder.push(
            r#"
            AND EXISTS (
                SELECT 1
                FROM asset_inventory_items AS inventory_items
                WHERE inventory_items.asset_id = assets.asset_id
                  AND inventory_items.location_id =
            "#,
        );
        builder.push_bind(location_id);
        builder.push(")");
    }
    if let Some(has_inventory) = query.has_inventory {
        if has_inventory {
            builder.push(
                " AND EXISTS (SELECT 1 FROM asset_inventory_items AS inventory_items WHERE inventory_items.asset_id = assets.asset_id)",
            );
        } else {
            builder.push(
                " AND NOT EXISTS (SELECT 1 FROM asset_inventory_items AS inventory_items WHERE inventory_items.asset_id = assets.asset_id)",
            );
        }
    }

    push_parameter_filters(builder, "assets.asset_id", parameter_filters);
}

fn include_parameters(query: &ListAssetsQuery) -> Result<bool, ListAssetsError> {
    let Some(include) = query.include.as_deref() else {
        return Ok(false);
    };
    let includes: Vec<_> = include
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    for include in &includes {
        if *include != "parameters" {
            return Err(ListAssetsError::ValidationError(format!(
                "Unsupported include: {include}"
            )));
        }
    }
    Ok(includes.contains(&"parameters"))
}

fn map_parameter_filter_error(error: ParameterFilterError) -> ListAssetsError {
    match error {
        ParameterFilterError::Validation(message) => ListAssetsError::ValidationError(message),
        ParameterFilterError::Unexpected(error) => ListAssetsError::UnexpectedError(error),
    }
}
