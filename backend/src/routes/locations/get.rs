use super::model::LocationResponse;
use super::queries::fetch_location;
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::domain::{LocationId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

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
    location_id: web::Path<Uuid>,
) -> Result<HttpResponse, GetLocationError> {
    let location_id: LocationId = location_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::Location,
        Action::Read(location_id.into()),
    )
    .await?
    {
        return Err(GetLocationError::Forbidden(
            "You don't have permission to view this location.".into(),
        ));
    }

    let location = fetch_location(&pool, location_id)
        .await?
        .ok_or(GetLocationError::NotFound("Location not found".into()))?;

    Ok(HttpResponse::Ok().json(LocationResponse::from(location)))
}
