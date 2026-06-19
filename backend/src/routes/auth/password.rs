use crate::access_control::get_actor;
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{AuthError, hash_password, validate_password_for_user};
use crate::domain::UserId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
    user_id: Uuid,
    previous_password_hash: String,
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

    let actor = get_actor(&pool, user_id)
        .await
        .map_err(ChangePasswordError::UnexpectedError)?
        .ok_or(ChangePasswordError::AuthenticationRequired)?;

    validate_password_for_user(*user_id, payload.current_password, &pool)
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
        .context("Failed to acquire a Postgres connection from the pool")?;
    let user =
        update_password_in_database(&mut transaction, *user_id, password_hash.expose_secret())
            .await?;

    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Update,
        AuditResource::User,
        Some(user.user_id),
        change_password_rollback_details(&user),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to change password.")?;

    Ok(HttpResponse::Ok().json(MessageResponse {
        message: "Password changed",
    }))
}

#[tracing::instrument(
    name = "Updating current user's password in the database",
    skip(transaction, password_hash),
    fields(user_id=%user_id)
)]
async fn update_password_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    password_hash: &str,
) -> Result<ChangedPasswordUser, ChangePasswordError> {
    sqlx::query_as!(
        ChangedPasswordUser,
        r#"
        WITH previous_user AS (
            SELECT user_id, password_hash AS previous_password_hash
            FROM users
            WHERE user_id = $1
            FOR UPDATE
        ),
        updated_user AS (
            UPDATE users
            SET password_hash = $2
            FROM previous_user
            WHERE users.user_id = previous_user.user_id
            RETURNING users.user_id
        )
        SELECT
            previous_user.user_id,
            previous_user.previous_password_hash
        FROM previous_user
        INNER JOIN updated_user USING (user_id)
        "#,
        user_id,
        password_hash,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(|e| ChangePasswordError::UnexpectedError(e.into()))
}

fn change_password_rollback_details(user: &ChangedPasswordUser) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "user",
            "where": {
                "user_id": user.user_id,
            },
            "values": {
                "password_hash": &user.previous_password_hash,
            },
        },
    })
}
