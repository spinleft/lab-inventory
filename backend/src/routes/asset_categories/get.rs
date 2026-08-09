use super::model::{
    AssetCategoryResponse, fetch_asset_category, fetch_asset_category_parameter_assignments,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::domain::{AssetCategoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum GetAssetCategoryError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GetAssetCategoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GetAssetCategoryError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetAssetCategoryError::Forbidden(_) => StatusCode::FORBIDDEN,
            GetAssetCategoryError::NotFound(_) => StatusCode::NOT_FOUND,
            GetAssetCategoryError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get an asset category",
    skip(pool),
    fields(actor_user_id=%actor_user_id, category_id=%category_id)
)]
pub async fn get_asset_category(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    category_id: web::Path<Uuid>,
) -> Result<HttpResponse, GetAssetCategoryError> {
    let category_id: AssetCategoryId = category_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::AssetCategory,
        Action::Read(category_id.into()),
    )
    .await?
    {
        return Err(GetAssetCategoryError::Forbidden(
            "You don't have permission to view this asset category.".into(),
        ));
    }

    let category =
        fetch_asset_category(&pool, category_id)
            .await?
            .ok_or(GetAssetCategoryError::NotFound(
                "Asset category not found".into(),
            ))?;
    let parameter_assignments =
        fetch_asset_category_parameter_assignments(&pool, category.category_id).await?;

    Ok(HttpResponse::Ok().json(AssetCategoryResponse::from_parts(
        category,
        parameter_assignments,
    )))
}
