use super::model::{LocationResponse, fetch_location};
use crate::access_control::{Actor, get_actor};
use crate::domain::{LaboratoryId, LocationId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::anyhow;
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum GetLocationError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GetLocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GetLocationError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetLocationError::Forbidden(_) => StatusCode::FORBIDDEN,
            GetLocationError::NotFound(_) => StatusCode::NOT_FOUND,
            GetLocationError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get a location",
    skip(pool),
    fields(actor_user_id=%actor_user_id, location_id=%location_id)
)]
pub async fn get_location(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    location_id: web::Path<LocationId>,
) -> Result<HttpResponse, GetLocationError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(GetLocationError::UnexpectedError)?
        .ok_or(GetLocationError::Forbidden(
            "Actor not found in the database".into(),
        ))?;

    let location = fetch_location(&pool, *location_id)
        .await?
        .ok_or(GetLocationError::NotFound("Location not found".into()))?;
    let laboratory_id = LaboratoryId::parse(location.laboratory_id)
        .map_err(|e| GetLocationError::UnexpectedError(anyhow!("{e}")))?;
    validate_read_permission(&actor, &laboratory_id)?;

    Ok(HttpResponse::Ok().json(LocationResponse::from(location)))
}

fn validate_read_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), GetLocationError> {
    if actor.can_read_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(GetLocationError::Forbidden(
            "You do not have permission to view this location".into(),
        ))
    }
}
