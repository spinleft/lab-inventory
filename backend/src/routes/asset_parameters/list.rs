use super::model::AssetParameterResponse;
use super::queries::{fetch_asset_parameter_options, fetch_asset_parameters};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::domain::{LaboratoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum ListAssetParametersError {
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListAssetParametersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListAssetParametersError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListAssetParametersError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListAssetParametersError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "List asset parameters",
    skip(pool),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn list_asset_parameters(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, ListAssetParametersError> {
    let laboratory_id: LaboratoryId = laboratory_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::AssetParameter,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListAssetParametersError::Forbidden(
            "You don't have permission to view asset parameters.".into(),
        ));
    }

    let parameters = fetch_asset_parameters(&pool, laboratory_id).await?;
    let mut response = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let options = fetch_asset_parameter_options(&pool, parameter.parameter_type_id).await?;
        response.push(AssetParameterResponse::from_parts(parameter, options));
    }

    Ok(HttpResponse::Ok().json(response))
}
