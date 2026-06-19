use super::model::{LaboratoryResponse, fetch_laboratory};
use crate::access_control::get_actor;
use crate::domain::UserId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum GetLaboratoryError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GetLaboratoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GetLaboratoryError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetLaboratoryError::Forbidden(_) => StatusCode::FORBIDDEN,
            GetLaboratoryError::NotFound(_) => StatusCode::NOT_FOUND,
            GetLaboratoryError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get a laboratory",
    skip(pool),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn get_laboratory(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, GetLaboratoryError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(GetLaboratoryError::UnexpectedError)?
        .ok_or(GetLaboratoryError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    if !actor.is_admin() {
        return Err(GetLaboratoryError::Forbidden(
            "You don't have permission to view this laboratory.".into(),
        ));
    }

    let laboratory = fetch_laboratory(&pool, *laboratory_id)
        .await?
        .ok_or(GetLaboratoryError::NotFound("Laboratory not found".into()))?;
    if actor.is_lab_admin() && actor.laboratory_id.map(Uuid::from) != Some(laboratory.laboratory_id)
    {
        return Err(GetLaboratoryError::Forbidden(
            "You don't have permission to view this laboratory.".into(),
        ));
    }

    Ok(HttpResponse::Ok().json(LaboratoryResponse::from(laboratory)))
}
