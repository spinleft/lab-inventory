use super::model::LaboratoryResponse;
use super::queries::fetch_laboratories;
use crate::access_control::{Action, Actor, ResourceType, validate_permission};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum ListLaboratoriesError {
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListLaboratoriesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListLaboratoriesError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListLaboratoriesError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListLaboratoriesError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(name = "List laboratories", skip(pool), fields(actor_user_id=%actor.user_id))]
pub async fn list_laboratories(
    actor: Actor,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ListLaboratoriesError> {
    // An admin that has not been bound to a laboratory yet administers nothing,
    // which is an empty list rather than a refusal.
    if actor.is_lab_admin() && actor.laboratory_id.is_none() {
        return Ok(HttpResponse::Ok().json(Vec::<LaboratoryResponse>::new()));
    }
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::Laboratory,
        Action::Browse(Uuid::nil()),
    )
    .await?
    {
        return Err(ListLaboratoriesError::Forbidden(
            "You don't have permission to list laboratories.".into(),
        ));
    }

    let laboratories: Vec<_> = fetch_laboratories(&pool, None)
        .await?
        .into_iter()
        .map(LaboratoryResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(laboratories))
}
