//! Every SQL statement the laboratory routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration
//! - functions never return a handler error type, only [`LaboratoryDatabaseError`],
//!   so any handler can reuse them
use super::model::LaboratoryRow;
use crate::utils::error_chain_fmt;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(super) enum LaboratoryDatabaseError {
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for LaboratoryDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> LaboratoryDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("laboratories_name_key")) => {
                return LaboratoryDatabaseError::Conflict("Laboratory name already exists".into());
            }
            (Some("23505"), _) => {
                return LaboratoryDatabaseError::Conflict("Laboratory already exists".into());
            }
            _ => {}
        }
    }

    LaboratoryDatabaseError::Unexpected(error.into())
}

/// A laboratory other rows still belong to cannot be removed. That is a conflict
/// rather than an unexpected failure, so the delete path maps the foreign key
/// violation on top of [`map_database_error`].
fn map_delete_error(error: sqlx::Error) -> LaboratoryDatabaseError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.code().as_deref() == Some("23503")
    {
        return LaboratoryDatabaseError::Conflict(
            "Laboratory is referenced by other records".into(),
        );
    }

    map_database_error(error)
}

pub(super) async fn fetch_laboratory(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Option<LaboratoryRow>, anyhow::Error> {
    sqlx::query_as!(
        LaboratoryRow,
        r#"
        SELECT laboratory_id, name, address, description, contact, created_at, updated_at
        FROM laboratories
        WHERE laboratory_id = $1
        "#,
        laboratory_id
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch laboratory")
}

/// Passing `None` lists every laboratory; passing an id narrows the result to
/// that one row, which is what a laboratory-scoped actor is allowed to see.
pub(super) async fn fetch_laboratories(
    pool: &PgPool,
    laboratory_id: Option<Uuid>,
) -> Result<Vec<LaboratoryRow>, anyhow::Error> {
    sqlx::query_as!(
        LaboratoryRow,
        r#"
        SELECT laboratory_id, name, address, description, contact, created_at, updated_at
        FROM laboratories
        WHERE $1::uuid IS NULL OR laboratory_id = $1
        ORDER BY name
        "#,
        laboratory_id,
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch laboratories")
}

#[tracing::instrument(
    name = "Saving new laboratory in the database",
    skip(transaction, name, address, description, contact)
)]
pub(super) async fn insert_laboratory(
    transaction: &mut Transaction<'_, Postgres>,
    name: &str,
    address: &str,
    description: Option<&str>,
    contact: Option<&str>,
) -> Result<LaboratoryRow, LaboratoryDatabaseError> {
    sqlx::query_as!(
        LaboratoryRow,
        r#"
        INSERT INTO laboratories (laboratory_id, name, address, description, contact)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING laboratory_id, name, address, description, contact, created_at, updated_at
        "#,
        Uuid::new_v4(),
        name,
        address,
        description,
        contact,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

/// `description` and `contact` are nullable, so a caller that wants to clear one
/// cannot be told apart from one that leaves it alone by the value alone. The
/// `should_update_*` flags carry that distinction into the statement.
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Updating laboratory in the database",
    skip(transaction, name, address, description, contact),
    fields(laboratory_id=%laboratory_id)
)]
pub(super) async fn update_laboratory_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    name: Option<&str>,
    address: Option<&str>,
    should_update_description: bool,
    description: Option<&str>,
    should_update_contact: bool,
    contact: Option<&str>,
) -> Result<LaboratoryRow, LaboratoryDatabaseError> {
    sqlx::query_as!(
        LaboratoryRow,
        r#"
        UPDATE laboratories
        SET
            name = COALESCE($2, name),
            address = COALESCE($3, address),
            description = CASE WHEN $4 THEN $5 ELSE description END,
            contact = CASE WHEN $6 THEN $7 ELSE contact END,
            updated_at = now()
        WHERE laboratory_id = $1
        RETURNING laboratory_id, name, address, description, contact, created_at, updated_at
        "#,
        laboratory_id,
        name,
        address,
        should_update_description,
        description,
        should_update_contact,
        contact,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

#[tracing::instrument(
    name = "Deleting laboratory from the database",
    skip(transaction),
    fields(laboratory_id=%laboratory_id)
)]
pub(super) async fn delete_laboratory_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
) -> Result<(), LaboratoryDatabaseError> {
    sqlx::query!(
        "DELETE FROM laboratories WHERE laboratory_id = $1",
        laboratory_id
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_delete_error)?;

    Ok(())
}
