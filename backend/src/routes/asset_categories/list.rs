use super::model::{
    AssetCategoryResponse, AssetCategoryRow, can_read_laboratory_categories, fetch_asset_category,
};
use crate::access_control::get_actor;
use crate::domain::{AssetCategoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct ListQuery {
    root_category_id: Option<AssetCategoryId>,
}

#[derive(thiserror::Error)]
pub enum ListAssetCategoriesError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListAssetCategoriesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListAssetCategoriesError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListAssetCategoriesError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListAssetCategoriesError::NotFound(_) => StatusCode::NOT_FOUND,
            ListAssetCategoriesError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "List asset categories",
    skip(pool, query),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn list_asset_categories(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, ListAssetCategoriesError> {
    let laboratory_id = laboratory_id.into_inner();
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(ListAssetCategoriesError::UnexpectedError)?
        .ok_or(ListAssetCategoriesError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    if !can_read_laboratory_categories(&actor, laboratory_id) {
        return Err(ListAssetCategoriesError::Forbidden(
            "You don't have permission to list asset categories for this laboratory.".into(),
        ));
    }

    let root_path = match query.root_category_id {
        Some(root_category_id) => {
            let root = fetch_asset_category(&pool, root_category_id).await?.ok_or(
                ListAssetCategoriesError::NotFound("Root asset category not found".into()),
            )?;
            if root.laboratory_id != laboratory_id {
                return Err(ListAssetCategoriesError::NotFound(
                    "Root asset category not found".into(),
                ));
            }
            Some(root.path)
        }
        None => None,
    };

    let categories: Vec<_> = fetch_asset_categories(&pool, laboratory_id, root_path.as_deref())
        .await?
        .into_iter()
        .map(AssetCategoryResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(categories))
}

async fn fetch_asset_categories(
    pool: &PgPool,
    laboratory_id: Uuid,
    root_path: Option<&str>,
) -> Result<Vec<AssetCategoryRow>, ListAssetCategoriesError> {
    sqlx::query_as!(
        AssetCategoryRow,
        r#"
        SELECT
            category_id,
            laboratory_id,
            parent_category_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        FROM asset_categories
        WHERE laboratory_id = $1
          AND ($2::text IS NULL OR path <@ $2::text::ltree)
        ORDER BY path
        "#,
        laboratory_id,
        root_path,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ListAssetCategoriesError::UnexpectedError(e.into()))
}
