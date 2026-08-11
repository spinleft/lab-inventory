use super::model::AssetParameterResponse;
use super::queries::{fetch_asset_parameter, fetch_asset_parameter_options};
use crate::access_control::AssetParameterPathId;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::AssetParameterId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
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
    fields(actor_user_id=%laboratory_context.actor().user_id, parameter_id=%parameter_id)
)]
pub async fn get_asset_parameter(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    parameter_id: AssetParameterPathId,
) -> Result<HttpResponse, GetAssetParameterError> {
    let actor = laboratory_context.authorization_actor();
    let parameter_id: AssetParameterId = parameter_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::AssetParameter,
        Action::Read(parameter_id.into()),
    )
    .await?
    {
        return Err(GetAssetParameterError::Forbidden(
            "You are not allowed to get this asset parameter.".into(),
        ));
    }

    let parameter = fetch_asset_parameter(&pool, parameter_id).await?.ok_or(
        GetAssetParameterError::NotFound("Asset parameter not found".into()),
    )?;

    let options = fetch_asset_parameter_options(&pool, parameter.parameter_type_id).await?;
    Ok(HttpResponse::Ok().json(AssetParameterResponse::from_parts(parameter, options)))
}
