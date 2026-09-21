//! Business flows that chain several statements together.
//!
//! Anything here orchestrates `queries.rs` and enforces rules that span more
//! than one row or table. Single-statement work belongs in `queries.rs`; HTTP
//! concerns belong in the handler modules.
use super::model::{AssetParameterOptionRow, AssetParameterValueRow};
use super::queries::{
    AssetParameterDatabaseError, delete_asset_parameter_options,
    delete_asset_parameter_value_from_database, delete_removed_asset_parameter_options,
    fetch_asset_parameter_option_id_by_code, fetch_asset_parameter_options_for_update,
    fetch_unit_dimension_for_update, insert_asset_parameter_option,
    insert_migrated_asset_parameter_option, insert_new_asset_parameter_option,
    migrate_asset_parameter_value_in_database, update_asset_parameter_option,
};
use crate::domain::{
    AssetParameterCode, AssetParameterDataType, AssetParameterOptionLabel, NewAssetParameterOption,
    UpdateAssetParameterOption, UpdateAssetParameterValueMigration,
};
use crate::routes::shared::parameter_values::{
    ParameterValueDefinition, ParameterValueError, resolve_parameter_value,
};
use sqlx::{Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

impl From<ParameterValueError> for AssetParameterDatabaseError {
    fn from(error: ParameterValueError) -> Self {
        match error {
            ParameterValueError::Validation(message) => Self::Validation(message),
            ParameterValueError::Unexpected(error) => Self::Unexpected(error),
        }
    }
}

/// Options only mean something for an enum parameter, and an enum parameter
/// without options could never be given a value.
pub(super) fn validate_new_options(
    data_type: AssetParameterDataType,
    options: &[NewAssetParameterOption],
) -> Result<(), AssetParameterDatabaseError> {
    if data_type != AssetParameterDataType::Enum && !options.is_empty() {
        return Err(AssetParameterDatabaseError::Validation(
            "Options are only allowed for enum asset parameters".into(),
        ));
    }
    if data_type == AssetParameterDataType::Enum && options.is_empty() {
        return Err(AssetParameterDatabaseError::Validation(
            "Enum asset parameters require at least one option".into(),
        ));
    }

    let mut seen_codes = HashSet::new();
    for option in options {
        if !seen_codes.insert(option.code.as_ref().to_string()) {
            return Err(AssetParameterDatabaseError::Validation(
                "Option codes must be unique".into(),
            ));
        }
    }

    Ok(())
}

/// The same rules as [`validate_new_options`], applied to an update. An update
/// that leaves the option list alone still has to end up with a usable one, so
/// the options already stored count towards the enum requirement.
pub(super) fn validate_updated_options(
    data_type: AssetParameterDataType,
    update_options: Option<&[UpdateAssetParameterOption]>,
    existing_options: &[AssetParameterOptionRow],
) -> Result<(), AssetParameterDatabaseError> {
    if data_type != AssetParameterDataType::Enum {
        if update_options.is_some_and(|options| !options.is_empty()) {
            return Err(AssetParameterDatabaseError::Validation(
                "Options are only allowed for enum asset parameters".into(),
            ));
        }
        return Ok(());
    }

    match update_options {
        Some(options) => validate_update_option_list(options),
        None => {
            if existing_options.is_empty() {
                Err(AssetParameterDatabaseError::Validation(
                    "Enum asset parameters require at least one option".into(),
                ))
            } else {
                Ok(())
            }
        }
    }
}

fn validate_update_option_list(
    options: &[UpdateAssetParameterOption],
) -> Result<(), AssetParameterDatabaseError> {
    if options.is_empty() {
        return Err(AssetParameterDatabaseError::Validation(
            "Enum asset parameters require at least one option".into(),
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut seen_codes = HashSet::new();
    for option in options {
        if let Some(option_id) = option.option_id
            && !seen_ids.insert(option_id)
        {
            return Err(AssetParameterDatabaseError::Validation(
                "Option ids must be unique".into(),
            ));
        }
        if !seen_codes.insert(option.code.as_ref().to_string()) {
            return Err(AssetParameterDatabaseError::Validation(
                "Option codes must be unique".into(),
            ));
        }
    }

    Ok(())
}

/// Settles what dimension the parameter's values are measured in.
///
/// Only numeric parameters carry units at all. When a default unit is given it
/// decides the dimension, and an explicitly requested dimension has to agree
/// with it.
pub(super) async fn normalize_unit_configuration(
    transaction: &mut Transaction<'_, Postgres>,
    data_type: AssetParameterDataType,
    unit_dimension: Option<&str>,
    default_unit_id: Option<Uuid>,
) -> Result<Option<String>, AssetParameterDatabaseError> {
    if !matches!(
        data_type,
        AssetParameterDataType::Number | AssetParameterDataType::Range
    ) {
        if unit_dimension.is_some() || default_unit_id.is_some() {
            return Err(AssetParameterDatabaseError::Validation(
                "Units are only allowed for number or range asset parameters".into(),
            ));
        }
        return Ok(None);
    }

    let Some(default_unit_id) = default_unit_id else {
        return Ok(unit_dimension.map(ToOwned::to_owned));
    };

    let default_unit_dimension = fetch_unit_dimension_for_update(transaction, default_unit_id)
        .await?
        .ok_or(AssetParameterDatabaseError::Validation(
            "Default unit not found".into(),
        ))?;

    if let Some(unit_dimension) = unit_dimension
        && unit_dimension != default_unit_dimension
    {
        return Err(AssetParameterDatabaseError::Validation(
            "Default unit dimension does not match asset parameter unit dimension".into(),
        ));
    }

    Ok(Some(default_unit_dimension))
}

/// Writes the options of a new parameter and returns them in the order the read
/// paths use, so a caller never has to re-read what it just wrote.
pub(super) async fn insert_new_options(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    options: &[NewAssetParameterOption],
) -> Result<Vec<AssetParameterOptionRow>, AssetParameterDatabaseError> {
    let mut rows = Vec::with_capacity(options.len());
    for option in options {
        let row = insert_new_asset_parameter_option(transaction, parameter_id, option).await?;
        rows.push(row);
    }

    rows.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then(left.label.cmp(&right.label))
            .then(left.code.cmp(&right.code))
    });

    Ok(rows)
}

/// Brings the stored options in line with the list the update sent.
///
/// An incoming option is matched to a stored one by id, or failing that by code,
/// so a caller may leave the ids out and still edit rather than replace. Whatever
/// is matched survives; everything else is dropped.
pub(super) async fn replace_options(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    existing_options: &[AssetParameterOptionRow],
    options: &[UpdateAssetParameterOption],
) -> Result<Vec<AssetParameterOptionRow>, AssetParameterDatabaseError> {
    let existing_by_id: HashMap<Uuid, &AssetParameterOptionRow> = existing_options
        .iter()
        .map(|option| (option.option_id, option))
        .collect();
    let existing_by_code: HashMap<&str, &AssetParameterOptionRow> = existing_options
        .iter()
        .map(|option| (option.code.as_str(), option))
        .collect();

    for option in options {
        if let Some(option_id) = option.option_id
            && !existing_by_id.contains_key(&option_id)
        {
            return Err(AssetParameterDatabaseError::Validation(
                "Asset parameter option not found".into(),
            ));
        }
    }

    let retained_existing_option_ids = options
        .iter()
        .filter_map(|option| {
            option.option_id.or_else(|| {
                existing_by_code
                    .get(option.code.as_ref())
                    .map(|existing| existing.option_id)
            })
        })
        .collect::<HashSet<_>>();

    delete_removed_asset_parameter_options(
        transaction,
        parameter_id,
        &retained_existing_option_ids,
    )
    .await?;
    for option in options {
        if let Some(option_id) = option.option_id {
            update_asset_parameter_option(transaction, option_id, option).await?;
        } else if let Some(existing) = existing_by_code.get(option.code.as_ref()) {
            update_asset_parameter_option(transaction, existing.option_id, option).await?;
        } else {
            insert_asset_parameter_option(transaction, parameter_id, option).await?;
        }
    }

    fetch_asset_parameter_options_for_update(transaction, parameter_id)
        .await
        .map_err(AssetParameterDatabaseError::Unexpected)
}

/// The option list a parameter ends up with after an update.
///
/// A parameter that is no longer an enum keeps no options at all; one that stays
/// an enum keeps what it had unless the update sent a replacement list.
pub(super) async fn apply_option_updates(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    data_type: AssetParameterDataType,
    existing_options: &[AssetParameterOptionRow],
    update_options: Option<&[UpdateAssetParameterOption]>,
) -> Result<Vec<AssetParameterOptionRow>, AssetParameterDatabaseError> {
    if data_type != AssetParameterDataType::Enum {
        delete_asset_parameter_options(transaction, parameter_id).await?;
        return Ok(Vec::new());
    }

    match update_options {
        Some(options) => {
            replace_options(transaction, parameter_id, existing_options, options).await
        }
        None => Ok(existing_options.to_vec()),
    }
}

/// Rewrites the parameter's stored values for a data type change.
///
/// The rows keep their identity: the migration rewrites the columns of the new
/// type in place and leaves the columns of the type they had before as they
/// were, so both readings live in the same row. A migration with no value
/// removes the row instead.
pub(super) async fn migrate_parameter_values(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    data_type: AssetParameterDataType,
    unit_dimension: Option<&str>,
    default_unit_id: Option<Uuid>,
    values: &[AssetParameterValueRow],
    migrations: &[UpdateAssetParameterValueMigration],
) -> Result<(), AssetParameterDatabaseError> {
    let definition = ParameterValueDefinition {
        parameter_type_id: parameter_id,
        data_type: data_type.as_str().to_string(),
        unit_dimension: unit_dimension.map(ToOwned::to_owned),
        default_unit_id,
    };
    let migrations_by_value_id: HashMap<Uuid, &UpdateAssetParameterValueMigration> = migrations
        .iter()
        .map(|migration| (migration.value_id, migration))
        .collect();

    /// A migration that lands on an enum option names it by code, because there
    /// is no id to name yet: the option either comes with the same request or is
    /// created here, with `option_label` as its label and the code as the
    /// fallback. Every other case is passed through untouched.
    async fn resolve_migration_target(
        transaction: &mut Transaction<'_, Postgres>,
        parameter_id: Uuid,
        data_type: AssetParameterDataType,
        value: &serde_json::Value,
    ) -> Result<serde_json::Value, AssetParameterDatabaseError> {
        if data_type != AssetParameterDataType::Enum
            || value.get("option_id").is_some()
            || value.get("option_code").is_none()
        {
            return Ok(value.clone());
        }

        let code = value
            .get("option_code")
            .and_then(serde_json::Value::as_str)
            .ok_or(AssetParameterDatabaseError::Validation(
                "Asset parameter option code must be a string".into(),
            ))?;
        if let Some(option_id) =
            fetch_asset_parameter_option_id_by_code(transaction, parameter_id, code).await?
        {
            return Ok(serde_json::json!({ "option_id": option_id }));
        }

        let label = match value.get("option_label") {
            Some(label) => label
                .as_str()
                .ok_or(AssetParameterDatabaseError::Validation(
                    "Asset parameter option label must be a string".into(),
                ))?,
            None => code,
        };
        let code = AssetParameterCode::parse(code.to_string())
            .map_err(AssetParameterDatabaseError::Validation)?;
        let label = AssetParameterOptionLabel::parse(label.to_string())
            .map_err(AssetParameterDatabaseError::Validation)?;
        let option_id = insert_migrated_asset_parameter_option(
            transaction,
            parameter_id,
            code.as_ref(),
            label.as_ref(),
        )
        .await?;

        Ok(serde_json::json!({ "option_id": option_id }))
    }

    for value in values {
        let Some(migration) = migrations_by_value_id.get(&value.value_id) else {
            return Err(AssetParameterDatabaseError::Validation(
                "All stored values must be migrated when the asset parameter data type changes"
                    .into(),
            ));
        };

        match migration.value.as_ref() {
            Some(target) => {
                let target =
                    resolve_migration_target(transaction, parameter_id, data_type, target).await?;
                let resolved = resolve_parameter_value(transaction, &definition, &target).await?;
                migrate_asset_parameter_value_in_database(
                    transaction,
                    value.value_id,
                    data_type,
                    &resolved,
                )
                .await?;
            }
            None => {
                delete_asset_parameter_value_from_database(transaction, value.value_id).await?;
            }
        }
    }

    Ok(())
}
