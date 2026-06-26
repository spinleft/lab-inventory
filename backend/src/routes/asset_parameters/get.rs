use super::model::{AssetParameterResponse, fetch_asset_parameter, fetch_asset_parameter_options};
use crate::access_control::{Actor, get_actor};
use crate::domain::{AssetParameterId, LaboratoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::anyhow;
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum GetAssetParameterError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GetAssetParameterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GetAssetParameterError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetAssetParameterError::Forbidden(_) => StatusCode::FORBIDDEN,
            GetAssetParameterError::NotFound(_) => StatusCode::NOT_FOUND,
            GetAssetParameterError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get an asset parameter",
    skip(pool),
    fields(actor_user_id=%actor_user_id, parameter_id=%parameter_id)
)]
pub async fn get_asset_parameter(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    parameter_id: web::Path<AssetParameterId>,
) -> Result<HttpResponse, GetAssetParameterError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(GetAssetParameterError::UnexpectedError)?
        .ok_or(GetAssetParameterError::Forbidden(
            "Actor not found in the database".into(),
        ))?;

    let parameter = fetch_asset_parameter(&pool, *parameter_id).await?.ok_or(
        GetAssetParameterError::NotFound("Asset parameter not found".into()),
    )?;
    let laboratory_id = LaboratoryId::parse(parameter.laboratory_id)
        .map_err(|e| GetAssetParameterError::UnexpectedError(anyhow!("{e}")))?;
    validate_read_permission(&actor, &laboratory_id)?;

    let options = fetch_asset_parameter_options(&pool, parameter.parameter_type_id).await?;
    Ok(HttpResponse::Ok().json(AssetParameterResponse::from_parts(parameter, options)))
}

fn validate_read_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), GetAssetParameterError> {
    if actor.can_query_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(GetAssetParameterError::Forbidden(
            "You do not have permission to view this asset parameter".into(),
        ))
    }
}
