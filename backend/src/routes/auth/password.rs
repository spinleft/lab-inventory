use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{
    AuthError, UserId, get_actor, hash_password, validate_password_for_user,
};
use crate::utils::{ApiError, error_chain_fmt};
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

#[derive(Serialize)]
struct MessageResponse {
    message: &'static str,
}

#[derive(sqlx::FromRow)]
struct ChangedPasswordUser {
    username: String,
    laboratory_id: Option<Uuid>,
}

#[derive(thiserror::Error)]
pub enum ChangePasswordError {
    #[error("Authentication required")]
    AuthenticationRequired,
    #[error("Current password is incorrect")]
    CurrentPasswordIncorrect,
    #[error("New password confirmation does not match")]
    PasswordConfirmationMismatch,
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ChangePasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ChangePasswordError {
    fn status_code(&self) -> StatusCode {
        match self {
            ChangePasswordError::AuthenticationRequired => StatusCode::UNAUTHORIZED,
            ChangePasswordError::CurrentPasswordIncorrect => StatusCode::UNAUTHORIZED,
            ChangePasswordError::PasswordConfirmationMismatch => StatusCode::BAD_REQUEST,
            ChangePasswordError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(serde_json::json!({
            "error": self.to_string()
        }))
    }
}

#[tracing::instrument(
    name = "Change current user's password",
    skip(pool, payload),
    fields(actor_user_id=%user_id)
)]
pub async fn change_password(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ChangePasswordError> {
    let payload = payload.into_inner();
    if payload.new_password.expose_secret() != payload.new_password_check.expose_secret() {
        return Err(ChangePasswordError::PasswordConfirmationMismatch);
    }

    let actor = get_actor(pool.get_ref(), user_id)
        .await
        .map_err(|e| match e {
            ApiError::Unauthorized => ChangePasswordError::AuthenticationRequired,
            _ => ChangePasswordError::UnexpectedError(e.into()),
        })?;

    validate_password_for_user(*user_id, payload.current_password, pool.get_ref())
        .await
        .map_err(|e| match e {
            AuthError::InvalidCredentials(_) => ChangePasswordError::CurrentPasswordIncorrect,
            AuthError::UnexpectedError(e) => ChangePasswordError::UnexpectedError(e),
        })?;

    let password_hash = hash_password(payload.new_password)
        .await
        .map_err(ChangePasswordError::UnexpectedError)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ChangePasswordError::UnexpectedError(e.into()))?;
    let user = sqlx::query_as::<_, ChangedPasswordUser>(
        r#"
        UPDATE users
        SET password_hash = $2
        WHERE user_id = $1
        RETURNING username, laboratory_id
        "#,
    )
    .bind(*user_id)
    .bind(password_hash.expose_secret())
    .fetch_one(transaction.as_mut())
    .await
    .map_err(|e| ChangePasswordError::UnexpectedError(e.into()))?;

    record_audit(
        &mut transaction,
        &actor,
        user.laboratory_id,
        AuditAction::Update,
        AuditResource::User,
        Some(*user_id),
        json!({ "username": user.username, "changed_fields": ["password"] }),
    )
    .await
    .map_err(|e| ChangePasswordError::UnexpectedError(e.into()))?;
    transaction
        .commit()
        .await
        .map_err(|e| ChangePasswordError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(MessageResponse {
        message: "Password changed",
    }))
}
