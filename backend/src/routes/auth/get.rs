use super::model::CurrentUser;
use super::queries::fetch_current_user;
use crate::domain::UserId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum MeError {
    #[error("Authentication required")]
    UnknownUser,
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for MeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for MeError {
    fn status_code(&self) -> StatusCode {
        match self {
            MeError::UnknownUser => StatusCode::UNAUTHORIZED,
            MeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(serde_json::json!({
            "error": self.to_string()
        }))
    }
}

#[tracing::instrument(name = "Get current user", skip(pool), fields(user_id=%user_id))]
pub async fn me(user_id: UserId, pool: web::Data<PgPool>) -> Result<HttpResponse, MeError> {
    // The session outlives the account it was opened for, so a session pointing
    // at a user that no longer exists is an authentication failure, not a 404.
    let row = fetch_current_user(&pool, *user_id)
        .await?
        .ok_or(MeError::UnknownUser)?;

    Ok(HttpResponse::Ok().json(CurrentUser::from(row)))
}
