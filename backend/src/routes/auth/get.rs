use crate::authentication::UserId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize)]
struct CurrentUser {
    user_id: Uuid,
    username: String,
    email: Option<String>,
    user_type: CurrentUserType,
    laboratory: Option<CurrentUserLaboratory>,
}

#[derive(Serialize)]
struct CurrentUserType {
    user_type_id: Uuid,
    name: String,
}

#[derive(Serialize)]
struct CurrentUserLaboratory {
    laboratory_id: Uuid,
    name: String,
}

#[derive(sqlx::FromRow)]
struct CurrentUserRow {
    user_id: Uuid,
    username: String,
    email: Option<String>,
    user_type_id: Uuid,
    user_type_name: String,
    laboratory_id: Option<Uuid>,
    laboratory_name: Option<String>,
}

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
    let row = sqlx::query_as::<_, CurrentUserRow>(
        r#"
        SELECT
            users.user_id,
            users.username,
            users.email,
            user_types.user_type_id,
            user_types.name AS user_type_name,
            laboratories.laboratory_id,
            laboratories.name AS laboratory_name
        FROM users
        INNER JOIN user_types USING (user_type_id)
        LEFT JOIN laboratories USING (laboratory_id)
        WHERE users.user_id = $1
        "#,
    )
    .bind(*user_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| MeError::UnexpectedError(e.into()))?
    .ok_or(MeError::UnknownUser)?;

    Ok(HttpResponse::Ok().json(CurrentUser {
        user_id: row.user_id,
        username: row.username,
        email: row.email,
        user_type: CurrentUserType {
            user_type_id: row.user_type_id,
            name: row.user_type_name,
        },
        laboratory: row
            .laboratory_id
            .zip(row.laboratory_name)
            .map(|(laboratory_id, name)| CurrentUserLaboratory {
                laboratory_id,
                name,
            }),
    }))
}
