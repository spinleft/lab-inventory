use super::model::{AssetParameterResponse, fetch_asset_parameter_options, parse_laboratory_id};
use crate::access_control::{Actor, get_actor};
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
    let laboratory_id = parse_laboratory_id(laboratory_id.into_inner())
        .map_err(ListAssetParametersError::UnexpectedError)?;
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(ListAssetParametersError::UnexpectedError)?
        .ok_or(ListAssetParametersError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_read_permission(&actor, &laboratory_id)?;

    let parameters = fetch_asset_parameters(&pool, laboratory_id).await?;
    let mut response = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let options = fetch_asset_parameter_options(&pool, parameter.parameter_type_id).await?;
        response.push(AssetParameterResponse::from_parts(parameter, options));
    }

    Ok(HttpResponse::Ok().json(response))
}

fn validate_read_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), ListAssetParametersError> {
    if actor.can_query_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(ListAssetParametersError::Forbidden(
            "You do not have permission to view asset parameters for this laboratory".into(),
        ))
    }
}

async fn fetch_asset_parameters(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
) -> Result<Vec<super::model::AssetParameterRow>, ListAssetParametersError> {
    sqlx::query_as::<_, super::model::AssetParameterRow>(
        r#"
        SELECT
            parameter_type_id,
            laboratory_id,
            code,
            name,
            data_type::text AS data_type,
            unit_dimension,
            default_unit_id,
            description,
            is_archived,
            created_at,
            updated_at
        FROM asset_parameter_types
        WHERE laboratory_id = $1
        ORDER BY code
        "#,
    )
    .bind(*laboratory_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ListAssetParametersError::UnexpectedError(e.into()))
}
