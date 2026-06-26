use super::model::{
    AssetResponse, fetch_asset, fetch_inventory_items_for_asset, fetch_parameter_values_for_asset,
};
use crate::access_control::{Actor, get_actor};
use crate::domain::{AssetId, LaboratoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::anyhow;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

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
    fields(actor_user_id=%actor_user_id, asset_id=%asset_id)
)]
pub async fn get_asset(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<AssetId>,
    query: web::Query<GetAssetQuery>,
) -> Result<HttpResponse, GetAssetError> {
    let include_parameters = include_parameters(&query)?;
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(GetAssetError::UnexpectedError)?
        .ok_or(GetAssetError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    let asset = fetch_asset(&pool, Uuid::from(*asset_id))
        .await?
        .ok_or(GetAssetError::NotFound("Asset not found".into()))?;
    let laboratory_id = LaboratoryId::parse(asset.laboratory_id)
        .map_err(|e| GetAssetError::UnexpectedError(anyhow!("{e}")))?;
    validate_read_permission(&actor, &laboratory_id)?;
    let include_internal_notes = actor.can_read_laboratory_resource(&laboratory_id);

    let inventory_items = fetch_inventory_items_for_asset(&pool, asset.asset_id).await?;
    let parameters = if include_parameters {
        Some(fetch_parameter_values_for_asset(&pool, asset.asset_id).await?)
    } else {
        None
    };

    Ok(
        HttpResponse::Ok().json(AssetResponse::from_parts_with_internal_notes(
            asset,
            Some(inventory_items),
            parameters,
            include_internal_notes,
        )),
    )
}

fn validate_read_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), GetAssetError> {
    if actor.can_query_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(GetAssetError::Forbidden(
            "You do not have permission to view this asset".into(),
        ))
    }
}

fn include_parameters(query: &GetAssetQuery) -> Result<bool, GetAssetError> {
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
            return Err(GetAssetError::ValidationError(format!(
                "Unsupported include: {include}"
            )));
        }
    }
    Ok(includes.contains(&"parameters"))
}
