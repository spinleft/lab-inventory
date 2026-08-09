use super::model::{DeletedUserRow, delete_user_rollback_details, fetch_user};
use crate::access_control::get_actor;
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::UserType;
use crate::domain::{UserId, UserRole};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum DeleteUserError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for DeleteUserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteUserError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteUserError::ValidationError(_) => StatusCode::BAD_REQUEST,
            DeleteUserError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteUserError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteUserError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Delete a user",
    skip(pool),
    fields(actor_user_id=%actor_user_id, target_user_id=%target_user_id)
)]
pub async fn delete_user(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    target_user_id: web::Path<Uuid>,
) -> Result<HttpResponse, DeleteUserError> {
    let target = fetch_user(&pool, *target_user_id).await?;
    let target_user_id = target.user_id.into();
    let target_user_type = target
        .user_type_name
        .as_ref()
        .map(UserType::parse)
        .transpose()
        .map_err(DeleteUserError::ValidationError)?
        .ok_or(DeleteUserError::UnexpectedError(anyhow::anyhow!(
            "User type is missing for the target user"
        )))?;
    let target_laboratory_id = target.laboratory_id.map(Uuid::into);
    let target_user_role = UserRole {
        user_type: target_user_type,
        laboratory_id: target_laboratory_id,
    };

    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(DeleteUserError::UnexpectedError)?
        .ok_or(DeleteUserError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    if actor.user_id == target_user_id {
        return Err(DeleteUserError::ValidationError(
            "Users cannot delete themselves".into(),
        ));
    }
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::User,
        Action::DeleteUser(&target_user_role),
    )
    .await?
    {
        return Err(DeleteUserError::Forbidden(
            "You don't have permission to delete this user.".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let deleted_user = delete_user_from_database(&mut transaction, target.user_id).await?;
    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Delete,
        AuditResource::User,
        Some(deleted_user.user_id),
        delete_user_rollback_details(&deleted_user),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete a user.")?;

    Ok(HttpResponse::NoContent().finish())
}

#[tracing::instrument(
    name = "Deleting user from the database",
    skip(transaction),
    fields(user_id=%user_id)
)]
async fn delete_user_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<DeletedUserRow, DeleteUserError> {
    sqlx::query_as!(
        DeletedUserRow,
        r#"
        WITH deleted_user AS (
            DELETE FROM users
            WHERE user_id = $1
            RETURNING
                users.user_id,
                users.username,
                users.password_hash,
                users.email,
                users.phone_number,
                users.user_type_id,
                users.laboratory_id,
                users.created_at,
                users.last_login_at
        )
        SELECT
            deleted_user.user_id,
            deleted_user.username,
            deleted_user.password_hash,
            deleted_user.email,
            deleted_user.phone_number,
            user_types.user_type_id AS "user_type_id?",
            user_types.name AS "user_type_name?",
            laboratories.laboratory_id AS "laboratory_id?",
            deleted_user.created_at,
            deleted_user.last_login_at
        FROM deleted_user
        INNER JOIN user_types USING (user_type_id)
        LEFT JOIN laboratories USING (laboratory_id)
        "#,
        user_id
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

fn map_database_error(error: sqlx::Error) -> DeleteUserError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23503") {
            return DeleteUserError::ConflictError("User is referenced by other records".into());
        }
    }

    DeleteUserError::UnexpectedError(error.into())
}
