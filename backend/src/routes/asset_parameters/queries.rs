//! Every SQL statement the asset parameter routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - functions never return a handler error type, only
//!   [`AssetParameterDatabaseError`], so any handler can reuse them
use super::model::{
    AssetParameterAssignmentRow, AssetParameterOptionRow, AssetParameterRow, AssetParameterValueRow,
};
use crate::domain::{
    AssetParameterDataType, AssetParameterId, LaboratoryId, NewAssetParameterOption,
    UpdateAssetParameterOption,
};
use crate::routes::shared::parameter_values::ResolvedParameterValue;
use crate::utils::error_chain_fmt;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(super) enum AssetParameterDatabaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for AssetParameterDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> AssetParameterDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("asset_parameter_types_laboratory_id_code_key")) => {
                return AssetParameterDatabaseError::Conflict(
                    "Asset parameter code already exists in this laboratory".into(),
                );
            }
            (Some("23505"), Some("asset_parameter_options_parameter_type_id_code_key")) => {
                return AssetParameterDatabaseError::Conflict(
                    "Asset parameter option code already exists".into(),
                );
            }
            (Some("23505"), _) => {
                return AssetParameterDatabaseError::Conflict(
                    "Asset parameter already exists".into(),
                );
            }
            (Some("23503"), _) => {
                return AssetParameterDatabaseError::Validation("Invalid referenced record".into());
            }
            (Some("23514"), _) => {
                return AssetParameterDatabaseError::Validation("Invalid asset parameter".into());
            }
            _ => {}
        }
    }

    AssetParameterDatabaseError::Unexpected(error.into())
}

/// A parameter other rows still carry values for cannot be removed. That is a
/// conflict rather than bad input, so the delete path maps the foreign key
/// violation differently from [`map_database_error`].
fn map_parameter_delete_error(error: sqlx::Error) -> AssetParameterDatabaseError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.code().as_deref() == Some("23503")
    {
        return AssetParameterDatabaseError::Conflict(
            "Asset parameter is referenced by other records".into(),
        );
    }

    map_database_error(error)
}

/// Dropping an option that assets already picked would orphan those values, so
/// the foreign key violation surfaces as a conflict here.
fn map_option_delete_error(error: sqlx::Error) -> AssetParameterDatabaseError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.code().as_deref() == Some("23503")
    {
        return AssetParameterDatabaseError::Conflict(
            "Asset parameter option is referenced by asset values".into(),
        );
    }

    map_database_error(error)
}

// ---------------------------------------------------------------------------
// asset parameters
// ---------------------------------------------------------------------------

pub(super) async fn fetch_asset_parameter(
    pool: &PgPool,
    parameter_id: AssetParameterId,
) -> Result<Option<AssetParameterRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetParameterRow>(
        r#"
        SELECT
            parameter_type_id,
            laboratory_id,
            code,
            name,
            data_type::text AS data_type,
            unit_dimension,
            default_unit_id,
            description,
            created_at,
            updated_at
        FROM asset_parameter_types
        WHERE parameter_type_id = $1
        "#,
    )
    .bind(Uuid::from(parameter_id))
    .fetch_optional(pool)
    .await
    .context("Failed to fetch asset parameter")
}

/// Same projection as [`fetch_asset_parameter`], but takes the row lock the
/// write paths need.
pub(super) async fn fetch_asset_parameter_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: AssetParameterId,
) -> Result<Option<AssetParameterRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetParameterRow>(
        r#"
        SELECT
            parameter_type_id,
            laboratory_id,
            code,
            name,
            data_type::text AS data_type,
            unit_dimension,
            default_unit_id,
            description,
            created_at,
            updated_at
        FROM asset_parameter_types
        WHERE parameter_type_id = $1
        FOR UPDATE
        "#,
    )
    .bind(Uuid::from(parameter_id))
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter for update")
}

pub(super) async fn fetch_asset_parameters(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
) -> Result<Vec<AssetParameterRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetParameterRow>(
        r#"
        SELECT
            parameter_type_id,
            laboratory_id,
            code,
            name,
            data_type::text AS data_type,
            unit_dimension,
            default_unit_id,
            description,
            created_at,
            updated_at
        FROM asset_parameter_types
        WHERE laboratory_id = $1
        ORDER BY code
        "#,
    )
    .bind(*laboratory_id)
    .fetch_all(pool)
    .await
    .context("Failed to fetch asset parameters")
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Saving new asset parameter in the database",
    skip(transaction, code, name, unit_dimension, description),
    fields(laboratory_id=%laboratory_id)
)]
pub(super) async fn insert_asset_parameter(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    code: &str,
    name: &str,
    data_type: AssetParameterDataType,
    unit_dimension: Option<&str>,
    default_unit_id: Option<Uuid>,
    description: Option<&str>,
) -> Result<AssetParameterRow, AssetParameterDatabaseError> {
    sqlx::query_as::<_, AssetParameterRow>(
        r#"
        INSERT INTO asset_parameter_types (
            parameter_type_id,
            laboratory_id,
            code,
            name,
            data_type,
            unit_dimension,
            default_unit_id,
            description
        )
        VALUES ($1, $2, $3, $4, $5::asset_parameter_data_type, $6, $7, $8)
        RETURNING
            parameter_type_id,
            laboratory_id,
            code,
            name,
            data_type::text AS data_type,
            unit_dimension,
            default_unit_id,
            description,
            created_at,
            updated_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(*laboratory_id)
    .bind(code)
    .bind(name)
    .bind(data_type.as_str())
    .bind(unit_dimension)
    .bind(default_unit_id)
    .bind(description)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Updating asset parameter in the database",
    skip(transaction, code, name, unit_dimension, description),
    fields(parameter_id=%parameter_id)
)]
pub(super) async fn update_asset_parameter_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    code: &str,
    name: &str,
    data_type: AssetParameterDataType,
    unit_dimension: Option<&str>,
    default_unit_id: Option<Uuid>,
    description: Option<&str>,
) -> Result<AssetParameterRow, AssetParameterDatabaseError> {
    sqlx::query_as::<_, AssetParameterRow>(
        r#"
        UPDATE asset_parameter_types
        SET
            code = $2,
            name = $3,
            data_type = $4::asset_parameter_data_type,
            unit_dimension = $5,
            default_unit_id = $6,
            description = $7,
            updated_at = now()
        WHERE parameter_type_id = $1
        RETURNING
            parameter_type_id,
            laboratory_id,
            code,
            name,
            data_type::text AS data_type,
            unit_dimension,
            default_unit_id,
            description,
            created_at,
            updated_at
        "#,
    )
    .bind(parameter_id)
    .bind(code)
    .bind(name)
    .bind(data_type.as_str())
    .bind(unit_dimension)
    .bind(default_unit_id)
    .bind(description)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

#[tracing::instrument(
    name = "Deleting asset parameter from the database",
    skip(transaction),
    fields(parameter_id=%parameter_id)
)]
pub(super) async fn delete_asset_parameter_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
) -> Result<(), AssetParameterDatabaseError> {
    sqlx::query("DELETE FROM asset_parameter_types WHERE parameter_type_id = $1")
        .bind(parameter_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_parameter_delete_error)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// enum options
// ---------------------------------------------------------------------------

pub(super) async fn fetch_asset_parameter_options(
    pool: &PgPool,
    parameter_id: Uuid,
) -> Result<Vec<AssetParameterOptionRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetParameterOptionRow>(
        r#"
        SELECT
            option_id,
            parameter_type_id,
            code,
            label,
            sort_order
        FROM asset_parameter_options
        WHERE parameter_type_id = $1
        ORDER BY sort_order, label, code
        "#,
    )
    .bind(parameter_id)
    .fetch_all(pool)
    .await
    .context("Failed to fetch asset parameter options")
}

pub(super) async fn fetch_asset_parameter_options_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
) -> Result<Vec<AssetParameterOptionRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetParameterOptionRow>(
        r#"
        SELECT
            option_id,
            parameter_type_id,
            code,
            label,
            sort_order
        FROM asset_parameter_options
        WHERE parameter_type_id = $1
        ORDER BY sort_order, label, code
        FOR UPDATE
        "#,
    )
    .bind(parameter_id)
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter options for update")
}

pub(super) async fn insert_new_asset_parameter_option(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    option: &NewAssetParameterOption,
) -> Result<AssetParameterOptionRow, AssetParameterDatabaseError> {
    sqlx::query_as::<_, AssetParameterOptionRow>(
        r#"
        INSERT INTO asset_parameter_options (
            option_id,
            parameter_type_id,
            code,
            label,
            sort_order
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING option_id, parameter_type_id, code, label, sort_order
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(parameter_id)
    .bind(option.code.as_ref())
    .bind(option.label.as_ref())
    .bind(option.sort_order)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

pub(super) async fn insert_asset_parameter_option(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    option: &UpdateAssetParameterOption,
) -> Result<(), AssetParameterDatabaseError> {
    sqlx::query(
        r#"
        INSERT INTO asset_parameter_options (
            option_id,
            parameter_type_id,
            code,
            label,
            sort_order
        )
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(parameter_id)
    .bind(option.code.as_ref())
    .bind(option.label.as_ref())
    .bind(option.sort_order)
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

/// Creates an option a value migration names but the parameter does not have
/// yet — the option a value lands on cannot have an id before the request that
/// introduces it.
#[tracing::instrument(
    name = "Inserting migrated asset parameter option in the database",
    skip(transaction, code, label),
    fields(parameter_id=%parameter_id)
)]
pub(super) async fn insert_migrated_asset_parameter_option(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    code: &str,
    label: &str,
) -> Result<Uuid, AssetParameterDatabaseError> {
    sqlx::query_scalar(
        r#"
        INSERT INTO asset_parameter_options (
            option_id,
            parameter_type_id,
            code,
            label,
            sort_order
        )
        VALUES ($1, $2, $3, $4, 0)
        RETURNING option_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(parameter_id)
    .bind(code)
    .bind(label)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

pub(super) async fn update_asset_parameter_option(
    transaction: &mut Transaction<'_, Postgres>,
    option_id: Uuid,
    option: &UpdateAssetParameterOption,
) -> Result<(), AssetParameterDatabaseError> {
    sqlx::query(
        r#"
        UPDATE asset_parameter_options
        SET
            code = $2,
            label = $3,
            sort_order = $4
        WHERE option_id = $1
        "#,
    )
    .bind(option_id)
    .bind(option.code.as_ref())
    .bind(option.label.as_ref())
    .bind(option.sort_order)
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

/// Removes every option of the parameter that the incoming list no longer keeps.
pub(super) async fn delete_removed_asset_parameter_options(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    retained_option_ids: &HashSet<Uuid>,
) -> Result<(), AssetParameterDatabaseError> {
    sqlx::query(
        r#"
        DELETE FROM asset_parameter_options
        WHERE parameter_type_id = $1
          AND option_id <> ALL($2)
        "#,
    )
    .bind(parameter_id)
    .bind(retained_option_ids.iter().copied().collect::<Vec<_>>())
    .execute(transaction.as_mut())
    .await
    .map_err(map_option_delete_error)?;

    Ok(())
}

pub(super) async fn delete_asset_parameter_options(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
) -> Result<(), AssetParameterDatabaseError> {
    sqlx::query("DELETE FROM asset_parameter_options WHERE parameter_type_id = $1")
        .bind(parameter_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_option_delete_error)?;

    Ok(())
}

pub(super) async fn fetch_asset_parameter_values(
    pool: &PgPool,
    parameter_id: AssetParameterId,
) -> Result<Vec<AssetParameterValueRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetParameterValueRow>(
        r#"
        SELECT
            asset_parameter_values.value_id,
            asset_parameter_values.asset_id,
            assets.name AS asset_name,
            assets.model AS asset_model,
            asset_parameter_values.parameter_type_id,
            asset_parameter_values.data_type::text AS data_type,
            asset_parameter_values.value_text,
            asset_parameter_values.value_number,
            asset_parameter_values.value_number_in_base,
            asset_parameter_values.value_range_start,
            asset_parameter_values.value_range_end,
            asset_parameter_values.value_range_start_in_base,
            asset_parameter_values.value_range_end_in_base,
            asset_parameter_values.unit_id,
            asset_parameter_values.value_boolean,
            asset_parameter_values.value_date,
            asset_parameter_values.value_option_id,
            asset_parameter_options.code AS option_code,
            asset_parameter_options.label AS option_label,
            asset_parameter_values.created_at,
            asset_parameter_values.updated_at
        FROM asset_parameter_values
        JOIN assets
          ON assets.asset_id = asset_parameter_values.asset_id
        LEFT JOIN asset_parameter_options
          ON asset_parameter_options.option_id = asset_parameter_values.value_option_id
        WHERE asset_parameter_values.parameter_type_id = $1
        ORDER BY assets.name, assets.model, asset_parameter_values.asset_id
        "#,
    )
    .bind(Uuid::from(parameter_id))
    .fetch_all(pool)
    .await
    .context("Failed to fetch asset parameter values")
}

pub(super) async fn fetch_asset_parameter_values_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
) -> Result<Vec<AssetParameterValueRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetParameterValueRow>(
        r#"
        SELECT
            asset_parameter_values.value_id,
            asset_parameter_values.asset_id,
            assets.name AS asset_name,
            assets.model AS asset_model,
            asset_parameter_values.parameter_type_id,
            asset_parameter_values.data_type::text AS data_type,
            asset_parameter_values.value_text,
            asset_parameter_values.value_number,
            asset_parameter_values.value_number_in_base,
            asset_parameter_values.value_range_start,
            asset_parameter_values.value_range_end,
            asset_parameter_values.value_range_start_in_base,
            asset_parameter_values.value_range_end_in_base,
            asset_parameter_values.unit_id,
            asset_parameter_values.value_boolean,
            asset_parameter_values.value_date,
            asset_parameter_values.value_option_id,
            asset_parameter_options.code AS option_code,
            asset_parameter_options.label AS option_label,
            asset_parameter_values.created_at,
            asset_parameter_values.updated_at
        FROM asset_parameter_values
        JOIN assets
          ON assets.asset_id = asset_parameter_values.asset_id
        LEFT JOIN asset_parameter_options
          ON asset_parameter_options.option_id = asset_parameter_values.value_option_id
        WHERE asset_parameter_values.parameter_type_id = $1
        ORDER BY assets.name, assets.model, asset_parameter_values.asset_id
        FOR UPDATE OF asset_parameter_values
        "#,
    )
    .bind(parameter_id)
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter values for update")
}

pub(super) async fn fetch_asset_parameter_option_id_by_code(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    code: &str,
) -> Result<Option<Uuid>, anyhow::Error> {
    sqlx::query_scalar(
        r#"
        SELECT option_id
        FROM asset_parameter_options
        WHERE parameter_type_id = $1
          AND code = $2
        "#,
    )
    .bind(parameter_id)
    .bind(code)
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter option by code")
}

pub(super) async fn fetch_asset_parameter_assignments_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
) -> Result<Vec<AssetParameterAssignmentRow>, anyhow::Error> {
    sqlx::query_as::<_, AssetParameterAssignmentRow>(
        r#"
        SELECT
            assignment_id,
            laboratory_id,
            parameter_type_id,
            category_id,
            asset_id,
            default_unit_id,
            applies_to_descendants,
            is_required,
            sort_order,
            created_at
        FROM asset_parameter_assignments
        WHERE parameter_type_id = $1
        ORDER BY created_at, assignment_id
        FOR UPDATE
        "#,
    )
    .bind(parameter_id)
    .fetch_all(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter assignments for update")
}

/// Rewrites one value row for a data type change.
///
/// Only the columns the new type owns are replaced; the columns of the type the
/// value had before keep their contents, so the row carries the pre-migration
/// value next to the migrated one. `value_option_id` is the exception: leaving
/// the enum type deletes the options it points at, and a leftover id would
/// violate the value-to-option foreign key, so it is cleared.
#[tracing::instrument(
    name = "Migrating asset parameter value in the database",
    skip(transaction, value),
    fields(value_id=%value_id)
)]
pub(super) async fn migrate_asset_parameter_value_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    value_id: Uuid,
    data_type: AssetParameterDataType,
    value: &ResolvedParameterValue,
) -> Result<(), AssetParameterDatabaseError> {
    sqlx::query(
        r#"
        UPDATE asset_parameter_values
        SET
            data_type = $2::asset_parameter_data_type,
            value_text = CASE WHEN $2 = 'text' THEN $3 ELSE value_text END,
            value_number = CASE WHEN $2 = 'number' THEN $4 ELSE value_number END,
            value_number_in_base = CASE WHEN $2 = 'number' THEN $5 ELSE value_number_in_base END,
            value_range_start = CASE WHEN $2 = 'range' THEN $6 ELSE value_range_start END,
            value_range_end = CASE WHEN $2 = 'range' THEN $7 ELSE value_range_end END,
            value_range_start_in_base = CASE WHEN $2 = 'range' THEN $8 ELSE value_range_start_in_base END,
            value_range_end_in_base = CASE WHEN $2 = 'range' THEN $9 ELSE value_range_end_in_base END,
            unit_id = CASE WHEN $2 IN ('number', 'range') THEN $10 ELSE unit_id END,
            value_boolean = CASE WHEN $2 = 'boolean' THEN $11 ELSE value_boolean END,
            value_date = CASE WHEN $2 = 'date' THEN $12 ELSE value_date END,
            value_option_id = CASE WHEN $2 = 'enum' THEN $13 ELSE NULL END,
            updated_at = now()
        WHERE value_id = $1
        "#,
    )
    .bind(value_id)
    .bind(data_type.as_str())
    .bind(value.value_text.as_deref())
    .bind(value.value_number)
    .bind(value.value_number_in_base)
    .bind(value.value_range_start)
    .bind(value.value_range_end)
    .bind(value.value_range_start_in_base)
    .bind(value.value_range_end_in_base)
    .bind(value.unit_id)
    .bind(value.value_boolean)
    .bind(value.value_date)
    .bind(value.value_option_id)
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

pub(super) async fn delete_asset_parameter_value_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    value_id: Uuid,
) -> Result<(), AssetParameterDatabaseError> {
    sqlx::query("DELETE FROM asset_parameter_values WHERE value_id = $1")
        .bind(value_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// units
// ---------------------------------------------------------------------------

pub(super) async fn fetch_unit_dimension_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> Result<Option<String>, anyhow::Error> {
    sqlx::query_scalar::<_, String>("SELECT dimension FROM units WHERE unit_id = $1 FOR UPDATE")
        .bind(unit_id)
        .fetch_optional(transaction.as_mut())
        .await
        .context("Failed to fetch unit dimension")
}
