use super::model::UserResponse;
use super::queries::fetch_user;
use crate::access_control::{Action, Actor, ResourceType, validate_permission};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum GetUserError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GetUserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GetUserError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetUserError::ValidationError(_) => StatusCode::BAD_REQUEST,
            GetUserError::Forbidden(_) => StatusCode::FORBIDDEN,
            GetUserError::NotFound(_) => StatusCode::NOT_FOUND,
            GetUserError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get a user",
    skip(pool),
    fields(actor_user_id=%actor.user_id, target_user_id=%target_user_id)
)]
pub async fn get_user(
    pool: web::Data<PgPool>,
    actor: Actor,
    target_user_id: web::Path<Uuid>,
) -> Result<HttpResponse, GetUserError> {
    let target_user = fetch_user(&pool, *target_user_id).await?;
    if !actor.is_system_admin() && actor.laboratory_id.map(Uuid::from) != target_user.laboratory_id
    {
        return Err(GetUserError::NotFound("User not found".into()));
    }
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::User,
        Action::Read(*target_user_id),
    )
    .await?
    {
        return Err(GetUserError::Forbidden(
            "You don't have permission to view this user.".to_string(),
        ));
    }

    Ok(HttpResponse::Ok().json(UserResponse::from(target_user)))
}
