use super::model::UnitResponse;
use super::queries::fetch_unit;
use crate::access_control::UnitPathId;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum GetUnitError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GetUnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GetUnitError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetUnitError::Forbidden(_) => StatusCode::FORBIDDEN,
            GetUnitError::NotFound(_) => StatusCode::NOT_FOUND,
            GetUnitError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get a unit",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, unit_id=%unit_id)
)]
pub async fn get_unit(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    unit_id: UnitPathId,
) -> Result<HttpResponse, GetUnitError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(&pool, &actor, ResourceType::Unit, Action::Read(*unit_id)).await? {
        return Err(GetUnitError::NotFound("Unit not found".into()));
    }

    let unit = fetch_unit(&pool, *unit_id)
        .await?
        .ok_or(GetUnitError::NotFound("Unit not found".into()))?;

    Ok(HttpResponse::Ok().json(UnitResponse::from(unit)))
}
