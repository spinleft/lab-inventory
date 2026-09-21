//! Turning a JSON parameter value into the column layout of
//! `asset_parameter_values`.
//!
//! The asset write paths and the asset-parameter data-type migration both have
//! to agree on which column a value ends up in and on the unit rules, so the
//! rules live here rather than in either of them.
use crate::domain::{AssetParameterDataType, AssetParameterValue};
use crate::utils::error_chain_fmt;
use anyhow::Context;
use chrono::NaiveDate;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(crate) enum ParameterValueError {
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for ParameterValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

/// The part of a parameter definition that decides how a value is written.
#[derive(Clone, sqlx::FromRow)]
pub(crate) struct ParameterValueDefinition {
    pub(crate) parameter_type_id: Uuid,
    pub(crate) data_type: String,
    pub(crate) unit_dimension: Option<String>,
    pub(crate) default_unit_id: Option<Uuid>,
}

#[derive(Clone, sqlx::FromRow)]
pub(crate) struct ParameterUnitRow {
    pub(crate) unit_id: Uuid,
    pub(crate) dimension: String,
    pub(crate) scale_to_base: f64,
}

/// A parameter value that has been validated and expanded into the column layout
/// of `asset_parameter_values`.
pub(crate) struct ResolvedParameterValue {
    pub(crate) parameter_type_id: Uuid,
    pub(crate) data_type: String,
    pub(crate) value_text: Option<String>,
    pub(crate) value_number: Option<f64>,
    pub(crate) value_number_in_base: Option<f64>,
    pub(crate) value_range_start: Option<f64>,
    pub(crate) value_range_end: Option<f64>,
    pub(crate) value_range_start_in_base: Option<f64>,
    pub(crate) value_range_end_in_base: Option<f64>,
    pub(crate) unit_id: Option<Uuid>,
    pub(crate) value_boolean: Option<bool>,
    pub(crate) value_date: Option<NaiveDate>,
    pub(crate) value_option_id: Option<Uuid>,
}

pub(crate) async fn resolve_parameter_value(
    transaction: &mut Transaction<'_, Postgres>,
    definition: &ParameterValueDefinition,
    value: &Value,
) -> Result<ResolvedParameterValue, ParameterValueError> {
    let data_type = AssetParameterDataType::parse(&definition.data_type)
        .map_err(|_| ParameterValueError::Validation("Invalid asset parameter data type".into()))?;
    let value =
        AssetParameterValue::parse(data_type, value).map_err(ParameterValueError::Validation)?;
    let mut resolved = ResolvedParameterValue {
        parameter_type_id: definition.parameter_type_id,
        data_type: definition.data_type.clone(),
        value_text: None,
        value_number: None,
        value_number_in_base: None,
        value_range_start: None,
        value_range_end: None,
        value_range_start_in_base: None,
        value_range_end_in_base: None,
        unit_id: None,
        value_boolean: None,
        value_date: None,
        value_option_id: None,
    };

    match value {
        AssetParameterValue::Text(text) => resolved.value_text = Some(text),
        AssetParameterValue::Number { number, unit_id } => {
            let (unit_id, number_base) =
                normalize_unit_value(transaction, definition, unit_id.map(Uuid::from), number)
                    .await?;
            resolved.value_number = Some(number);
            resolved.value_number_in_base = number_base;
            resolved.unit_id = unit_id;
        }
        AssetParameterValue::Range {
            start,
            end,
            unit_id,
        } => {
            let (unit_id, start_base) =
                normalize_unit_value(transaction, definition, unit_id.map(Uuid::from), start)
                    .await?;
            let (_, end_base) = normalize_unit_value(transaction, definition, unit_id, end).await?;
            resolved.value_range_start = Some(start);
            resolved.value_range_end = Some(end);
            resolved.value_range_start_in_base = start_base;
            resolved.value_range_end_in_base = end_base;
            resolved.unit_id = unit_id;
        }
        AssetParameterValue::Boolean(boolean) => resolved.value_boolean = Some(boolean),
        AssetParameterValue::Date(date) => resolved.value_date = Some(date),
        AssetParameterValue::Enum(option_id) => {
            validate_option(transaction, definition.parameter_type_id, option_id).await?;
            resolved.value_option_id = Some(option_id);
        }
    }

    Ok(resolved)
}

async fn normalize_unit_value(
    transaction: &mut Transaction<'_, Postgres>,
    definition: &ParameterValueDefinition,
    unit_id: Option<Uuid>,
    value: f64,
) -> Result<(Option<Uuid>, Option<f64>), ParameterValueError> {
    let unit_id = unit_id.or(definition.default_unit_id);
    let Some(unit_id) = unit_id else {
        return Ok((None, None));
    };
    let unit = fetch_unit(transaction, unit_id).await?;

    match definition.unit_dimension.as_deref() {
        Some(unit_dimension) if unit_dimension == unit.dimension => {
            Ok((Some(unit.unit_id), Some(value * unit.scale_to_base)))
        }
        Some(_) => Err(ParameterValueError::Validation(
            "Parameter value unit dimension does not match parameter definition".into(),
        )),
        None => Err(ParameterValueError::Validation(
            "Parameter value unit is not allowed for this parameter".into(),
        )),
    }
}

pub(crate) async fn validate_option(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_type_id: Uuid,
    option_id: Uuid,
) -> Result<(), ParameterValueError> {
    let found = sqlx::query_scalar!(
        r#"
        SELECT option_id
        FROM asset_parameter_options
        WHERE parameter_type_id = $1
          AND option_id = $2
        FOR UPDATE
        "#,
        parameter_type_id,
        option_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch asset parameter option")?;

    if found.is_some() {
        Ok(())
    } else {
        Err(ParameterValueError::Validation(
            "Asset parameter option not found".into(),
        ))
    }
}

pub(crate) async fn fetch_unit(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> Result<ParameterUnitRow, ParameterValueError> {
    sqlx::query_as!(
        ParameterUnitRow,
        r#"
        SELECT unit_id, dimension, scale_to_base
        FROM units
        WHERE unit_id = $1
        FOR UPDATE
        "#,
        unit_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch unit")?
    .ok_or(ParameterValueError::Validation("Unit not found".into()))
}
