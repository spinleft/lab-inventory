use super::model::{AssetCategoryResponse, fetch_asset_category};
use crate::access_control::{Actor, get_actor};
use crate::domain::{AssetCategoryId, LaboratoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::anyhow;
use sqlx::PgPool;

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
    category_id: web::Path<AssetCategoryId>,
) -> Result<HttpResponse, GetAssetCategoryError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(GetAssetCategoryError::UnexpectedError)?
        .ok_or(GetAssetCategoryError::Forbidden(
            "Actor not found in the database".into(),
        ))?;

    let category =
        fetch_asset_category(&pool, *category_id)
            .await?
            .ok_or(GetAssetCategoryError::NotFound(
                "Asset category not found".into(),
            ))?;
    let laboratory_id = LaboratoryId::parse(category.laboratory_id)
        .map_err(|e| GetAssetCategoryError::UnexpectedError(anyhow!("{e}")))?;
    validate_read_permission(&actor, &laboratory_id)?;

    Ok(HttpResponse::Ok().json(AssetCategoryResponse::from(category)))
}

fn validate_read_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), GetAssetCategoryError> {
    if actor.can_read_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(GetAssetCategoryError::Forbidden(
            "You do not have permission to view this asset category".into(),
        ))
    }
}
