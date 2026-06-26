use crate::domain::LaboratoryId;
use anyhow::Context;
use chrono::NaiveDate;
use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(crate) enum ParameterFilterError {
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for ParameterFilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        crate::utils::error_chain_fmt(self, f)
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParameterFilterInput {
    parameter_type_id: Uuid,
    text: Option<String>,
    number_min: Option<f64>,
    number_max: Option<f64>,
    range_start: Option<f64>,
    range_end: Option<f64>,
    boolean: Option<bool>,
    date_start: Option<NaiveDate>,
    date_end: Option<NaiveDate>,
    option_id: Option<Uuid>,
    unit_id: Option<Uuid>,
}

#[derive(Clone)]
pub(crate) enum ParameterFilter {
    Text {
        parameter_type_id: Uuid,
        text: String,
    },
    Number {
        parameter_type_id: Uuid,
        min: Option<f64>,
        max: Option<f64>,
        compare_base: bool,
    },
    Range {
        parameter_type_id: Uuid,
        start: f64,
        end: f64,
        compare_base: bool,
    },
    Boolean {
        parameter_type_id: Uuid,
        value: bool,
    },
    Date {
        parameter_type_id: Uuid,
        start: Option<NaiveDate>,
        end: Option<NaiveDate>,
    },
    Enum {
        parameter_type_id: Uuid,
        option_id: Uuid,
    },
}

#[derive(Clone, sqlx::FromRow)]
struct ParameterDefinitionRow {
    parameter_type_id: Uuid,
    data_type: String,
    unit_dimension: Option<String>,
    default_unit_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct UnitRow {
    dimension: String,
    scale_to_base: f64,
}

pub(crate) async fn parse_parameter_filters(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    raw: Option<&str>,
) -> Result<Vec<ParameterFilter>, ParameterFilterError> {
    let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Ok(Vec::new());
    };
    let inputs: Vec<ParameterFilterInput> = serde_json::from_str(raw).map_err(|_| {
        ParameterFilterError::Validation("parameter_filters must be a JSON array".into())
    })?;
    if inputs.len() > 20 {
        return Err(ParameterFilterError::Validation(
            "parameter_filters cannot contain more than 20 conditions".into(),
        ));
    }
    if inputs.is_empty() {
        return Ok(Vec::new());
    }

    let mut seen = HashSet::new();
    let parameter_type_ids = inputs
        .iter()
        .map(|input| {
            if !seen.insert(input.parameter_type_id) {
                return Err(ParameterFilterError::Validation(
                    "parameter_filters cannot contain duplicate parameter_type_id values".into(),
                ));
            }
            Ok(input.parameter_type_id)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let definitions = fetch_parameter_definitions(pool, laboratory_id, &parameter_type_ids).await?;
    let mut filters = Vec::with_capacity(inputs.len());
    for input in inputs {
        let definition = definitions.get(&input.parameter_type_id).ok_or_else(|| {
            ParameterFilterError::Validation(
                "parameter_filters contain an unknown parameter_type_id".into(),
            )
        })?;
        filters.push(normalize_filter(pool, input, definition).await?);
    }
    Ok(filters)
}

pub(crate) fn push_parameter_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    asset_id_expression: &str,
    filters: &[ParameterFilter],
) {
    for filter in filters {
        builder.push(
            " AND EXISTS (SELECT 1 FROM asset_parameter_values AS parameter_values WHERE parameter_values.asset_id = ",
        );
        builder.push(asset_id_expression);
        builder.push(" AND parameter_values.parameter_type_id = ");
        builder.push_bind(filter.parameter_type_id());

        match filter {
            ParameterFilter::Text { text, .. } => {
                builder.push(" AND parameter_values.value_text ILIKE ");
                builder.push_bind(format!("%{text}%"));
            }
            ParameterFilter::Number {
                min,
                max,
                compare_base,
                ..
            } => {
                let column = if *compare_base {
                    "COALESCE(parameter_values.value_number_base, parameter_values.value_number)"
                } else {
                    "parameter_values.value_number"
                };
                if let Some(min) = min {
                    builder.push(" AND ");
                    builder.push(column);
                    builder.push(" >= ");
                    builder.push_bind(*min);
                }
                if let Some(max) = max {
                    builder.push(" AND ");
                    builder.push(column);
                    builder.push(" <= ");
                    builder.push_bind(*max);
                }
            }
            ParameterFilter::Range {
                start,
                end,
                compare_base,
                ..
            } => {
                let start_column = if *compare_base {
                    "COALESCE(parameter_values.value_range_start_base, parameter_values.value_range_start)"
                } else {
                    "parameter_values.value_range_start"
                };
                let end_column = if *compare_base {
                    "COALESCE(parameter_values.value_range_end_base, parameter_values.value_range_end)"
                } else {
                    "parameter_values.value_range_end"
                };
                builder.push(" AND ");
                builder.push(start_column);
                builder.push(" <= ");
                builder.push_bind(*start);
                builder.push(" AND ");
                builder.push(end_column);
                builder.push(" >= ");
                builder.push_bind(*end);
            }
            ParameterFilter::Boolean { value, .. } => {
                builder.push(" AND parameter_values.value_boolean = ");
                builder.push_bind(*value);
            }
            ParameterFilter::Date { start, end, .. } => {
                if let Some(start) = start {
                    builder.push(" AND parameter_values.value_date >= ");
                    builder.push_bind(*start);
                }
                if let Some(end) = end {
                    builder.push(" AND parameter_values.value_date <= ");
                    builder.push_bind(*end);
                }
            }
            ParameterFilter::Enum { option_id, .. } => {
                builder.push(" AND parameter_values.value_option_id = ");
                builder.push_bind(*option_id);
            }
        }

        builder.push(")");
    }
}

impl ParameterFilter {
    fn parameter_type_id(&self) -> Uuid {
        match self {
            ParameterFilter::Text {
                parameter_type_id, ..
            }
            | ParameterFilter::Number {
                parameter_type_id, ..
            }
            | ParameterFilter::Range {
                parameter_type_id, ..
            }
            | ParameterFilter::Boolean {
                parameter_type_id, ..
            }
            | ParameterFilter::Date {
                parameter_type_id, ..
            }
            | ParameterFilter::Enum {
                parameter_type_id, ..
            } => *parameter_type_id,
        }
    }
}

async fn fetch_parameter_definitions(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    parameter_type_ids: &[Uuid],
) -> Result<HashMap<Uuid, ParameterDefinitionRow>, ParameterFilterError> {
    let rows = sqlx::query_as::<_, ParameterDefinitionRow>(
        r#"
        SELECT
            parameter_type_id,
            data_type::text AS data_type,
            unit_dimension,
            default_unit_id
        FROM asset_parameter_types
        WHERE laboratory_id = $1
          AND parameter_type_id = ANY($2)
        "#,
    )
    .bind(*laboratory_id)
    .bind(parameter_type_ids)
    .fetch_all(pool)
    .await
    .context("Failed to fetch parameter definitions for query filters")?;

    Ok(rows
        .into_iter()
        .map(|row| (row.parameter_type_id, row))
        .collect())
}

async fn normalize_filter(
    pool: &PgPool,
    input: ParameterFilterInput,
    definition: &ParameterDefinitionRow,
) -> Result<ParameterFilter, ParameterFilterError> {
    match definition.data_type.as_str() {
        "text" => {
            let text = input.text.unwrap_or_default().trim().to_string();
            if text.is_empty() {
                return Err(ParameterFilterError::Validation(
                    "Text parameter filters require text".into(),
                ));
            }
            Ok(ParameterFilter::Text {
                parameter_type_id: input.parameter_type_id,
                text,
            })
        }
        "number" => {
            if input.number_min.is_none() && input.number_max.is_none() {
                return Err(ParameterFilterError::Validation(
                    "Number parameter filters require number_min or number_max".into(),
                ));
            }
            if input
                .number_min
                .zip(input.number_max)
                .is_some_and(|(min, max)| min > max)
            {
                return Err(ParameterFilterError::Validation(
                    "number_min cannot exceed number_max".into(),
                ));
            }
            let unit = resolve_filter_unit(pool, definition, input.unit_id).await?;
            Ok(ParameterFilter::Number {
                parameter_type_id: input.parameter_type_id,
                min: scale_optional(input.number_min, unit.as_ref()),
                max: scale_optional(input.number_max, unit.as_ref()),
                compare_base: unit.is_some(),
            })
        }
        "range" => {
            let Some(start) = input.range_start else {
                return Err(ParameterFilterError::Validation(
                    "Range parameter filters require range_start".into(),
                ));
            };
            let Some(end) = input.range_end else {
                return Err(ParameterFilterError::Validation(
                    "Range parameter filters require range_end".into(),
                ));
            };
            if start > end {
                return Err(ParameterFilterError::Validation(
                    "range_start cannot exceed range_end".into(),
                ));
            }
            let unit = resolve_filter_unit(pool, definition, input.unit_id).await?;
            Ok(ParameterFilter::Range {
                parameter_type_id: input.parameter_type_id,
                start: scale_value(start, unit.as_ref()),
                end: scale_value(end, unit.as_ref()),
                compare_base: unit.is_some(),
            })
        }
        "boolean" => input
            .boolean
            .map(|value| ParameterFilter::Boolean {
                parameter_type_id: input.parameter_type_id,
                value,
            })
            .ok_or_else(|| {
                ParameterFilterError::Validation("Boolean parameter filters require boolean".into())
            }),
        "date" => {
            if input.date_start.is_none() && input.date_end.is_none() {
                return Err(ParameterFilterError::Validation(
                    "Date parameter filters require date_start or date_end".into(),
                ));
            }
            if input
                .date_start
                .zip(input.date_end)
                .is_some_and(|(start, end)| start > end)
            {
                return Err(ParameterFilterError::Validation(
                    "date_start cannot be after date_end".into(),
                ));
            }
            Ok(ParameterFilter::Date {
                parameter_type_id: input.parameter_type_id,
                start: input.date_start,
                end: input.date_end,
            })
        }
        "enum" => input
            .option_id
            .map(|option_id| ParameterFilter::Enum {
                parameter_type_id: input.parameter_type_id,
                option_id,
            })
            .ok_or_else(|| {
                ParameterFilterError::Validation("Enum parameter filters require option_id".into())
            }),
        _ => Err(ParameterFilterError::Validation(
            "Invalid asset parameter data type".into(),
        )),
    }
}

async fn resolve_filter_unit(
    pool: &PgPool,
    definition: &ParameterDefinitionRow,
    requested_unit_id: Option<Uuid>,
) -> Result<Option<UnitRow>, ParameterFilterError> {
    let unit_id = requested_unit_id.or(definition.default_unit_id);
    let Some(unit_id) = unit_id else {
        if definition.unit_dimension.is_some() {
            return Err(ParameterFilterError::Validation(
                "Unit-based parameter filters require a unit_id or default unit".into(),
            ));
        }
        return Ok(None);
    };
    let unit = sqlx::query_as::<_, UnitRow>(
        r#"
        SELECT dimension, scale_to_base
        FROM units
        WHERE unit_id = $1
        "#,
    )
    .bind(unit_id)
    .fetch_optional(pool)
    .await
    .context("Failed to fetch unit for parameter query filter")?
    .ok_or_else(|| ParameterFilterError::Validation("Unit not found".into()))?;

    if definition.unit_dimension.as_deref() != Some(unit.dimension.as_str()) {
        return Err(ParameterFilterError::Validation(
            "Parameter filter unit dimension does not match parameter definition".into(),
        ));
    }
    Ok(Some(unit))
}

fn scale_optional(value: Option<f64>, unit: Option<&UnitRow>) -> Option<f64> {
    value.map(|value| scale_value(value, unit))
}

fn scale_value(value: f64, unit: Option<&UnitRow>) -> f64 {
    unit.map_or(value, |unit| value * unit.scale_to_base)
}
