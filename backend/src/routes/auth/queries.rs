//! Every SQL statement the auth routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration
//! - functions never return a handler error type, so any handler can reuse them.
//!   Nothing here maps a constraint violation to a message of its own, so plain
//!   [`anyhow::Error`] is enough
//!
//! Verifying a password is not here: that belongs to `authentication`, which
//! owns the hashing.
use super::model::{ChangedPasswordUser, CurrentUserRow};
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(super) async fn fetch_current_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<CurrentUserRow>, anyhow::Error> {
    sqlx::query_as::<_, CurrentUserRow>(
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
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to fetch the current user")
}

pub(super) async fn touch_last_login(pool: &PgPool, user_id: Uuid) -> Result<(), anyhow::Error> {
    sqlx::query!(
        "UPDATE users SET last_login_at = now() WHERE user_id = $1",
        user_id
    )
    .execute(pool)
    .await
    .context("Failed to record the login timestamp")?;

    Ok(())
}

/// Swaps the stored hash and hands back the one it replaced, so the audit entry
/// holds what is needed to undo the change.
#[tracing::instrument(
    name = "Updating current user's password in the database",
    skip(transaction, password_hash),
    fields(user_id=%user_id)
)]
pub(super) async fn update_password_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    password_hash: &str,
) -> Result<ChangedPasswordUser, anyhow::Error> {
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
    .context("Failed to change the password")
}
