use super::model::AssetParameterValueResponse;
use super::queries::fetch_asset_parameter_values;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::AssetParameterId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum ListAssetParameterValuesError {
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListAssetParameterValuesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListAssetParameterValuesError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListAssetParameterValuesError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListAssetParameterValuesError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Every value the parameter is stored on, which is what a data type change has
/// to migrate: the screen in front of it edits one row per asset.
#[tracing::instrument(
    name = "List asset parameter values",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, parameter_id=%parameter_id)
)]
pub async fn list_asset_parameter_values(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    parameter_id: AssetParameterId,
) -> Result<HttpResponse, ListAssetParameterValuesError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::AssetParameter,
        Action::Read(parameter_id.into()),
    )
    .await?
    {
        return Err(ListAssetParameterValuesError::Forbidden(
            "You are not allowed to read this asset parameter.".into(),
        ));
    }

    let values = fetch_asset_parameter_values(&pool, parameter_id).await?;
    let response: Vec<AssetParameterValueResponse> = values
        .into_iter()
        .map(AssetParameterValueResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(response))
}
