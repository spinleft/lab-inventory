use super::model::{UnitResponse, fetch_unit};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::domain::UserId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

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
    fields(actor_user_id=%actor_user_id, unit_id=%unit_id)
)]
pub async fn get_unit(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    unit_id: web::Path<Uuid>,
) -> Result<HttpResponse, GetUnitError> {
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::Unit,
        Action::Read(*unit_id),
    )
    .await?
    {
        return Err(GetUnitError::Forbidden(
            "You don't have permission to view this unit.".into(),
        ));
    }

    let unit = fetch_unit(&pool, *unit_id)
        .await?
        .ok_or(GetUnitError::NotFound("Unit not found".into()))?;

    Ok(HttpResponse::Ok().json(UnitResponse::from(unit)))
}
