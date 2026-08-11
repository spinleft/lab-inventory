use super::model::{AssetResponse, parse_include};
use super::queries::{
    fetch_asset, fetch_inventory_items_for_asset, fetch_parameter_values_for_asset,
};
use crate::access_control::AssetPathId;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::AssetId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct GetAssetQuery {
    include: Option<String>,
}

#[derive(thiserror::Error)]
pub enum GetAssetError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GetAssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GetAssetError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetAssetError::ValidationError(_) => StatusCode::BAD_REQUEST,
            GetAssetError::Forbidden(_) => StatusCode::FORBIDDEN,
            GetAssetError::NotFound(_) => StatusCode::NOT_FOUND,
            GetAssetError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get an asset",
    skip(pool, query),
    fields(actor_user_id=%laboratory_context.actor().user_id, asset_id=%asset_id)
)]
pub async fn get_asset(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    asset_id: AssetPathId,
    query: web::Query<GetAssetQuery>,
) -> Result<HttpResponse, GetAssetError> {
    let actor = laboratory_context.authorization_actor();
    let asset_id: AssetId = asset_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::Asset,
        Action::Read(asset_id.into()),
    )
    .await?
    {
        return Err(GetAssetError::Forbidden(
            "You are not allowed to get this asset".into(),
        ));
    }

    let include_parameters =
        parse_include(query.include.as_deref()).map_err(GetAssetError::ValidationError)?;
    let asset = fetch_asset(&pool, asset_id.into())
        .await?
        .ok_or(GetAssetError::NotFound("Asset not found".into()))?;
    let include_internal_notes = validate_permission(
        &pool,
        &actor,
        ResourceType::Asset,
        Action::BrowseInternal(asset.laboratory_id),
    )
    .await?;

    let inventory_items = fetch_inventory_items_for_asset(&pool, asset.asset_id).await?;
    let parameters = if include_parameters {
        Some(fetch_parameter_values_for_asset(&pool, asset.asset_id).await?)
    } else {
        None
    };

    Ok(HttpResponse::Ok().json(AssetResponse::from_parts(
        asset,
        Some(inventory_items),
        parameters,
        include_internal_notes,
    )))
}
