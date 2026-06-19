use super::model::{UserResponse, UserRow};
use crate::access_control::get_actor;
use crate::domain::UserId;
use crate::domain::{LaboratoryId, UserType};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::anyhow;
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
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(ListUsersError::UnexpectedError)?
        .ok_or(ListUsersError::Forbidden(
            "Actor not found in the database".into(),
        ))?;

    let mut users = Vec::new();
    for user in fetch_users(&pool).await? {
        if actor.can_view_user(
            UserId::parse(user.user_id).map_err(|e| ListUsersError::UnexpectedError(anyhow!(e)))?,
            UserType::parse(user.user_type_name.as_ref().ok_or(
                ListUsersError::ValidationError("User type is required".into()),
            )?)
            .map_err(|e| ListUsersError::UnexpectedError(anyhow!(e)))?,
            user.laboratory_id
                .map(|id| {
                    LaboratoryId::parse(id).map_err(|e| ListUsersError::UnexpectedError(anyhow!(e)))
                })
                .transpose()?,
        ) {
            users.push(UserResponse::from(user));
        }
    }

    Ok(HttpResponse::Ok().json(users))
}

async fn fetch_users(pool: &PgPool) -> Result<Vec<UserRow>, ListUsersError> {
    sqlx::query_as!(
        UserRow,
        r#"
        SELECT
            users.user_id,
            users.username,
            users.email,
            users.phone_number,
            user_types.user_type_id AS "user_type_id?",
            user_types.name AS "user_type_name?",
            laboratories.laboratory_id AS "laboratory_id?",
            laboratories.name AS "laboratory_name?",
            users.created_at,
            users.last_login_at
        FROM users
        INNER JOIN user_types USING (user_type_id)
        LEFT JOIN laboratories USING (laboratory_id)
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ListUsersError::UnexpectedError(e.into()))
}
