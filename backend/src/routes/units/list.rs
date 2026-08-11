use super::model::UnitResponse;
use super::queries::fetch_units;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum ListUnitsError {
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListUnitsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListUnitsError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListUnitsError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListUnitsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(name = "List units", skip(pool), fields(actor_user_id=%laboratory_context.actor().user_id))]
pub async fn list_units(
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
) -> Result<HttpResponse, ListUnitsError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    if !validate_permission(
        &pool,
        actor,
        ResourceType::Unit,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListUnitsError::Forbidden(
            "You don't have permission to view units.".into(),
        ));
    }

    let units: Vec<_> = fetch_units(&pool, laboratory_id)
        .await?
        .into_iter()
        .map(UnitResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(units))
}
