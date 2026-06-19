use super::model::{UserResponse, fetch_user};
use crate::access_control::get_actor;
use crate::domain::UserId;
use crate::domain::{LaboratoryId, UserType};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::anyhow;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum GetUserError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
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
            GetUserError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get a user",
    skip(pool),
    fields(actor_user_id=%actor_user_id, target_user_id=%target_user_id)
)]
pub async fn get_user(
    pool: web::Data<PgPool>,
    actor_user_id: UserId,
    target_user_id: web::Path<Uuid>,
) -> Result<HttpResponse, GetUserError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(GetUserError::UnexpectedError)?
        .ok_or(GetUserError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    let target_user = fetch_user(&pool, *target_user_id).await?;
    actor
        .can_view_user(
            UserId::parse(target_user.user_id)
                .map_err(|e| GetUserError::UnexpectedError(anyhow!(e)))?,
            UserType::parse(target_user.user_type_name.as_ref().ok_or(
                GetUserError::ValidationError("User type is required".into()),
            )?)
            .map_err(|e| GetUserError::UnexpectedError(anyhow!(e)))?,
            target_user
                .laboratory_id
                .map(|id| {
                    LaboratoryId::parse(id).map_err(|e| GetUserError::UnexpectedError(anyhow!(e)))
                })
                .transpose()?,
        )
        .then_some(())
        .ok_or(GetUserError::Forbidden(
            "You don't have permission to view this user.".to_string(),
        ))?;

    Ok(HttpResponse::Ok().json(UserResponse::from(target_user)))
}
