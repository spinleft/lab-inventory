use super::model::UserResponse;
use super::queries::fetch_users;
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::domain::UserId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum ListUsersError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListUsersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListUsersError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListUsersError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ListUsersError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListUsersError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(name = "List users", skip(pool), fields(actor_user_id=%actor_user_id))]
pub async fn list_users(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ListUsersError> {
    let mut users = Vec::new();
    for user in fetch_users(&pool).await? {
        if validate_permission(
            &pool,
            &actor_user_id,
            ResourceType::User,
            Action::Read(user.user_id),
        )
        .await?
        {
            users.push(UserResponse::from(user));
        }
    }

    Ok(HttpResponse::Ok().json(users))
}
