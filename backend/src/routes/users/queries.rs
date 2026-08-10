//! Every SQL statement the user routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration
//! - functions never return a handler error type, only [`UserDatabaseError`],
//!   so any handler can reuse them
use super::model::{DeletedUserRow, UserRow};
use crate::domain::NewUser;
use crate::utils::error_chain_fmt;
use anyhow::{Context, anyhow};
use secrecy::{ExposeSecret, Secret};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(in crate::routes) enum UserDatabaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for UserDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> UserDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("users_username_key")) => {
                return UserDatabaseError::Conflict("Username already exists".into());
            }
            (Some("23505"), Some("users_email_key")) => {
                return UserDatabaseError::Conflict("Email already exists".into());
            }
            (Some("23505"), Some("users_phone_number_key")) => {
                return UserDatabaseError::Conflict("Phone number already exists".into());
            }
            (Some("23505"), _) => {
                return UserDatabaseError::Conflict("User already exists".into());
            }
            (Some("23503"), Some("users_laboratory_id_fkey")) => {
                return UserDatabaseError::Validation("Invalid laboratory".into());
            }
            (Some("23503"), Some("users_user_type_id_fkey")) => {
                return UserDatabaseError::Validation("Invalid user type".into());
            }
            (Some("23503"), _) => {
                return UserDatabaseError::Validation("Invalid referenced record".into());
            }
            _ => {}
        }
    }

    UserDatabaseError::Unexpected(error.into())
}

/// A user other rows still point at cannot be removed. That is a conflict rather
/// than bad input, so the delete path maps the foreign key violation differently
/// from [`map_database_error`].
fn map_delete_error(error: sqlx::Error) -> UserDatabaseError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.code().as_deref() == Some("23503")
    {
        return UserDatabaseError::Conflict("User is referenced by other records".into());
    }

    map_database_error(error)
}

pub(super) async fn fetch_user(pool: &PgPool, user_id: Uuid) -> Result<UserRow, anyhow::Error> {
    sqlx::query_as!(
        UserRow,
        r#"
        SELECT
            users.user_id,
            users.username,
            users.email,
            users.phone_number,
            user_types.user_type_id,
            user_types.name AS user_type_name,
            laboratories.laboratory_id AS "laboratory_id?",
            laboratories.name AS "laboratory_name?",
            users.created_at,
            users.last_login_at
        FROM users
        INNER JOIN user_types USING (user_type_id)
        LEFT JOIN laboratories USING (laboratory_id)
        WHERE users.user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch user")?
    .ok_or(anyhow!("User not found"))
}

pub(super) async fn fetch_users(pool: &PgPool) -> Result<Vec<UserRow>, anyhow::Error> {
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
    .context("Failed to fetch users")
}

/// The user type is stored by id but written by name, so the insert resolves the
/// name in the same statement and joins the row back to build the response.
#[tracing::instrument(
    name = "Saving new user in the database",
    skip(new_user, password_hash, transaction)
)]
pub(super) async fn insert_user(
    transaction: &mut Transaction<'_, Postgres>,
    new_user: NewUser,
    password_hash: Secret<String>,
) -> Result<UserRow, UserDatabaseError> {
    let new_user_id = Uuid::new_v4();
    let username = new_user.username.as_ref();
    let user_type_name = new_user.user_type.to_string();
    let laboratory_id = new_user.laboratory_id.map(Uuid::from);
    let email = new_user.email.map(String::from);
    let phone_number = new_user.phone_number.map(String::from);

    sqlx::query_as!(
        UserRow,
        r#"
        WITH inserted_user AS (
            INSERT INTO users (user_id, username, password_hash, user_type_id, laboratory_id, email, phone_number)
            SELECT $1, $2, $3, user_types.user_type_id, $4, $5, $6
            FROM user_types
            WHERE user_types.name = $7
            RETURNING
                users.user_id,
                users.username,
                users.email,
                users.phone_number,
                users.user_type_id,
                users.laboratory_id,
                users.created_at,
                users.last_login_at
        )
        SELECT
            inserted_user.user_id,
            inserted_user.username,
            inserted_user.email,
            inserted_user.phone_number,
            user_types.user_type_id AS "user_type_id?",
            user_types.name AS "user_type_name?",
            laboratories.laboratory_id AS "laboratory_id?",
            laboratories.name AS "laboratory_name?",
            inserted_user.created_at,
            inserted_user.last_login_at
        FROM inserted_user
        INNER JOIN user_types USING (user_type_id)
        LEFT JOIN laboratories USING (laboratory_id)
        "#,
        new_user_id,
        username,
        password_hash.expose_secret(),
        laboratory_id,
        email,
        phone_number,
        &user_type_name,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

#[tracing::instrument(
    name = "Updating user in the database",
    skip(transaction, username, email, phone_number),
    fields(user_id=%user_id)
)]
pub(super) async fn update_user_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    username: Option<&str>,
    user_type_name: &str,
    laboratory_id: Option<Uuid>,
    email: Option<&str>,
    phone_number: Option<&str>,
) -> Result<UserRow, UserDatabaseError> {
    sqlx::query_as!(
        UserRow,
        r#"
        WITH updated_user AS (
            UPDATE users
            SET
                username = COALESCE($2, username),
                user_type_id = (SELECT user_type_id FROM user_types WHERE name = $3),
                laboratory_id = $4,
                email = $5,
                phone_number = $6
            WHERE user_id = $1
            RETURNING
                users.user_id,
                users.username,
                users.email,
                users.phone_number,
                users.user_type_id,
                users.laboratory_id,
                users.created_at,
                users.last_login_at
        )
        SELECT
            updated_user.user_id,
            updated_user.username,
            updated_user.email,
            updated_user.phone_number,
            user_types.user_type_id AS "user_type_id?",
            user_types.name AS "user_type_name?",
            laboratories.laboratory_id AS "laboratory_id?",
            laboratories.name AS "laboratory_name?",
            updated_user.created_at,
            updated_user.last_login_at
        FROM updated_user
        INNER JOIN user_types USING (user_type_id)
        LEFT JOIN laboratories USING (laboratory_id)
        "#,
        user_id,
        username,
        user_type_name,
        laboratory_id,
        email,
        phone_number,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

/// Returns the row as it was before the delete, including the password hash, so
/// the audit entry holds everything needed to recreate the user.
#[tracing::instrument(
    name = "Deleting user from the database",
    skip(transaction),
    fields(user_id=%user_id)
)]
pub(super) async fn delete_user_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<DeletedUserRow, UserDatabaseError> {
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
    .map_err(map_delete_error)
}
