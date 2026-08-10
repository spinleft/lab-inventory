//! Every SQL statement the unit routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration
//! - functions never return a handler error type, only [`UnitDatabaseError`],
//!   so any handler can reuse them
use super::model::UnitRow;
use crate::domain::{LaboratoryId, NewUnit};
use crate::utils::error_chain_fmt;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(super) enum UnitDatabaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for UnitDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> UnitDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("units_laboratory_id_code_key")) => {
                return UnitDatabaseError::Conflict("Unit code already exists".into());
            }
            (Some("23505"), _) => {
                return UnitDatabaseError::Conflict("Unit already exists".into());
            }
            (Some("23514"), _) => {
                return UnitDatabaseError::Validation("Invalid unit".into());
            }
            (Some("23503"), Some("units_dimension_fkey")) => {
                return UnitDatabaseError::Validation("Invalid unit dimension".into());
            }
            (Some("23503"), _) => {
                return UnitDatabaseError::Validation("Invalid referenced record".into());
            }
            _ => {}
        }
    }

    UnitDatabaseError::Unexpected(error.into())
}

/// A unit other rows still measure in cannot be removed. That is a conflict
/// rather than bad input, so the delete path maps the foreign key violation
/// differently from [`map_database_error`].
fn map_delete_error(error: sqlx::Error) -> UnitDatabaseError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.code().as_deref() == Some("23503")
    {
        return UnitDatabaseError::Conflict("Unit is referenced by other records".into());
    }

    map_database_error(error)
}

pub(super) async fn fetch_unit(
    pool: &PgPool,
    unit_id: Uuid,
) -> Result<Option<UnitRow>, anyhow::Error> {
    sqlx::query_as!(
        UnitRow,
        r#"
        SELECT unit_id, laboratory_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        FROM units
        WHERE unit_id = $1
        "#,
        unit_id,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch unit")
}

/// Same projection as [`fetch_unit`], but takes the row lock the write paths
/// need. `query_as!` requires a literal, so the column list cannot be shared.
pub(super) async fn fetch_unit_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> Result<Option<UnitRow>, anyhow::Error> {
    sqlx::query_as!(
        UnitRow,
        r#"
        SELECT unit_id, laboratory_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        FROM units
        WHERE unit_id = $1
        FOR UPDATE
        "#,
        unit_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch unit for update")
}

pub(super) async fn fetch_units(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
) -> Result<Vec<UnitRow>, anyhow::Error> {
    sqlx::query_as!(
        UnitRow,
        r#"
        SELECT unit_id, laboratory_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        FROM units
        WHERE laboratory_id = $1
        ORDER BY dimension, code
        "#,
        Uuid::from(laboratory_id),
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch units")
}

#[tracing::instrument(name = "Saving new unit in the database", skip(transaction, new_unit))]
pub(super) async fn insert_unit(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    new_unit: &NewUnit,
) -> Result<UnitRow, UnitDatabaseError> {
    let dimension = new_unit.dimension.to_string();
    sqlx::query_as!(
        UnitRow,
        r#"
        INSERT INTO units (unit_id, laboratory_id, code, name, symbol, dimension, scale_to_base, allow_decimal)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING unit_id, laboratory_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        "#,
        Uuid::new_v4(),
        laboratory_id,
        new_unit.code.as_ref(),
        new_unit.name.as_ref(),
        new_unit.symbol.as_ref(),
        &dimension,
        new_unit.scale_to_base,
        new_unit.allow_decimal,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Updating unit in the database",
    skip(transaction, code, name, symbol, dimension),
    fields(unit_id=%unit_id)
)]
pub(super) async fn update_unit_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
    code: Option<&str>,
    name: Option<&str>,
    symbol: Option<&str>,
    dimension: Option<&str>,
    scale_to_base: Option<f64>,
    allow_decimal: Option<bool>,
) -> Result<UnitRow, UnitDatabaseError> {
    sqlx::query_as!(
        UnitRow,
        r#"
        UPDATE units
        SET
            code = COALESCE($2, code),
            name = COALESCE($3, name),
            symbol = COALESCE($4, symbol),
            dimension = COALESCE($5, dimension),
            scale_to_base = COALESCE($6, scale_to_base),
            allow_decimal = COALESCE($7, allow_decimal)
        WHERE unit_id = $1
        RETURNING unit_id, laboratory_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        "#,
        unit_id,
        code,
        name,
        symbol,
        dimension,
        scale_to_base,
        allow_decimal,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

#[tracing::instrument(name = "Deleting unit from the database", skip(transaction), fields(unit_id=%unit_id))]
pub(super) async fn delete_unit_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> Result<(), UnitDatabaseError> {
    sqlx::query!("DELETE FROM units WHERE unit_id = $1", unit_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_delete_error)?;

    Ok(())
}
