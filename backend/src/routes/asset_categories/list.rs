use super::model::AssetCategoryResponse;
use super::queries::{
    fetch_asset_categories, fetch_asset_category,
    fetch_asset_category_parameter_assignments_for_categories,
};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::AssetCategoryId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use serde::Deserialize;
use sqlx::PgPool;

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
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn list_asset_categories(
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, ListAssetCategoriesError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    if !validate_permission(
        &pool,
        actor,
        ResourceType::AssetCategory,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListAssetCategoriesError::Forbidden(
            "You don't have permission to view asset categories.".into(),
        ));
    }

    let root_path = match query.root_category_id {
        Some(root_category_id) => {
            let root = fetch_asset_category(&pool, root_category_id).await?.ok_or(
                ListAssetCategoriesError::NotFound("Root asset category not found".into()),
            )?;
            if root.laboratory_id != *laboratory_id {
                return Err(ListAssetCategoriesError::NotFound(
                    "Root asset category not found".into(),
                ));
            }
            Some(root.path)
        }
        None => None,
    };

    let categories = fetch_asset_categories(&pool, laboratory_id, root_path.as_deref()).await?;
    let category_ids: Vec<_> = categories
        .iter()
        .map(|category| category.category_id)
        .collect();
    let mut assignments_by_category_id =
        fetch_asset_category_parameter_assignments_for_categories(&pool, &category_ids).await?;
    let categories: Vec<_> = categories
        .into_iter()
        .map(|category| {
            let parameter_assignments = assignments_by_category_id
                .remove(&category.category_id)
                .unwrap_or_default();
            AssetCategoryResponse::from_parts(category, parameter_assignments)
        })
        .collect();

    Ok(HttpResponse::Ok().json(categories))
}
