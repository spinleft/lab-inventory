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

const MAX_FILTERS: usize = 20;

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug)]
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

#[derive(Clone, sqlx::FromRow)]
struct UnitRow {
    unit_id: Uuid,
    dimension: String,
    scale_to_base: f64,
}

pub(crate) async fn parse_parameter_filters(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    raw: Option<&str>,
) -> Result<Vec<ParameterFilter>, ParameterFilterError> {
    let inputs = parse_filter_inputs(raw)?;
    if inputs.is_empty() {
        return Ok(Vec::new());
    }

    let parameter_type_ids: Vec<_> = inputs.iter().map(|input| input.parameter_type_id).collect();
    let definitions = fetch_parameter_definitions(pool, laboratory_id, &parameter_type_ids).await?;
    let units = fetch_filter_units(pool, &required_unit_ids(&inputs, &definitions)).await?;

    inputs
        .into_iter()
        .map(|input| {
            let definition = definitions.get(&input.parameter_type_id).ok_or_else(|| {
                ParameterFilterError::Validation(
                    "parameter_filters contain an unknown parameter_type_id".into(),
                )
            })?;
            normalize_filter(input, definition, &units)
        })
        .collect()
}

fn parse_filter_inputs(
    raw: Option<&str>,
) -> Result<Vec<ParameterFilterInput>, ParameterFilterError> {
    let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Ok(Vec::new());
    };
    let inputs: Vec<ParameterFilterInput> = serde_json::from_str(raw).map_err(|_| {
        ParameterFilterError::Validation("parameter_filters must be a JSON array".into())
    })?;
    if inputs.len() > MAX_FILTERS {
        return Err(ParameterFilterError::Validation(format!(
            "parameter_filters cannot contain more than {MAX_FILTERS} conditions"
        )));
    }

    let mut seen = HashSet::new();
    for input in &inputs {
        if !seen.insert(input.parameter_type_id) {
            return Err(ParameterFilterError::Validation(
                "parameter_filters cannot contain duplicate parameter_type_id values".into(),
            ));
        }
    }
    Ok(inputs)
}

/// Units are only consulted for the numeric data types, so pre-fetching anything
/// else would surface unit errors for filters that never look at a unit.
fn required_unit_ids(
    inputs: &[ParameterFilterInput],
    definitions: &HashMap<Uuid, ParameterDefinitionRow>,
) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    inputs
        .iter()
        .filter_map(|input| {
            let definition = definitions.get(&input.parameter_type_id)?;
            if !matches!(definition.data_type.as_str(), "number" | "range") {
                return None;
            }
            filter_unit_id(definition, input.unit_id)
        })
        .filter(|unit_id| seen.insert(*unit_id))
        .collect()
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
                    "COALESCE(parameter_values.value_number_in_base, parameter_values.value_number)"
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
                    "COALESCE(parameter_values.value_range_start_in_base, parameter_values.value_range_start)"
                } else {
                    "parameter_values.value_range_start"
                };
                let end_column = if *compare_base {
                    "COALESCE(parameter_values.value_range_end_in_base, parameter_values.value_range_end)"
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

async fn fetch_filter_units(
    pool: &PgPool,
    unit_ids: &[Uuid],
) -> Result<HashMap<Uuid, UnitRow>, ParameterFilterError> {
    if unit_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, UnitRow>(
        r#"
        SELECT unit_id, dimension, scale_to_base
        FROM units
        WHERE unit_id = ANY($1)
        "#,
    )
    .bind(unit_ids)
    .fetch_all(pool)
    .await
    .context("Failed to fetch units for parameter query filters")?;

    Ok(rows.into_iter().map(|row| (row.unit_id, row)).collect())
}

fn normalize_filter(
    input: ParameterFilterInput,
    definition: &ParameterDefinitionRow,
    units: &HashMap<Uuid, UnitRow>,
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
            let unit = resolve_filter_unit(definition, input.unit_id, units)?;
            Ok(ParameterFilter::Number {
                parameter_type_id: input.parameter_type_id,
                min: scale_optional(input.number_min, unit),
                max: scale_optional(input.number_max, unit),
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
            let unit = resolve_filter_unit(definition, input.unit_id, units)?;
            Ok(ParameterFilter::Range {
                parameter_type_id: input.parameter_type_id,
                start: scale_value(start, unit),
                end: scale_value(end, unit),
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

fn filter_unit_id(
    definition: &ParameterDefinitionRow,
    requested_unit_id: Option<Uuid>,
) -> Option<Uuid> {
    requested_unit_id.or(definition.default_unit_id)
}

fn resolve_filter_unit<'a>(
    definition: &ParameterDefinitionRow,
    requested_unit_id: Option<Uuid>,
    units: &'a HashMap<Uuid, UnitRow>,
) -> Result<Option<&'a UnitRow>, ParameterFilterError> {
    let Some(unit_id) = filter_unit_id(definition, requested_unit_id) else {
        if definition.unit_dimension.is_some() {
            return Err(ParameterFilterError::Validation(
                "Unit-based parameter filters require a unit_id or default unit".into(),
            ));
        }
        return Ok(None);
    };
    let unit = units
        .get(&unit_id)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(data_type: &str) -> ParameterDefinitionRow {
        ParameterDefinitionRow {
            parameter_type_id: Uuid::new_v4(),
            data_type: data_type.into(),
            unit_dimension: None,
            default_unit_id: None,
        }
    }

    fn input(definition: &ParameterDefinitionRow) -> ParameterFilterInput {
        ParameterFilterInput {
            parameter_type_id: definition.parameter_type_id,
            text: None,
            number_min: None,
            number_max: None,
            range_start: None,
            range_end: None,
            boolean: None,
            date_start: None,
            date_end: None,
            option_id: None,
            unit_id: None,
        }
    }

    fn unit(dimension: &str, scale_to_base: f64) -> UnitRow {
        UnitRow {
            unit_id: Uuid::new_v4(),
            dimension: dimension.into(),
            scale_to_base,
        }
    }

    fn units(rows: Vec<UnitRow>) -> HashMap<Uuid, UnitRow> {
        rows.into_iter().map(|row| (row.unit_id, row)).collect()
    }

    fn validation_message(error: ParameterFilterError) -> String {
        match error {
            ParameterFilterError::Validation(message) => message,
            ParameterFilterError::Unexpected(error) => {
                panic!("expected a validation error, got {error}")
            }
        }
    }

    #[test]
    fn blank_input_yields_no_filters() {
        for raw in [None, Some(""), Some("   ")] {
            assert!(parse_filter_inputs(raw).unwrap().is_empty());
        }
    }

    #[test]
    fn non_array_input_is_rejected() {
        let error = parse_filter_inputs(Some("not json")).unwrap_err();
        assert_eq!(
            validation_message(error),
            "parameter_filters must be a JSON array"
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let raw = format!(r#"[{{"parameter_type_id":"{}","nope":1}}]"#, Uuid::new_v4());
        let error = parse_filter_inputs(Some(&raw)).unwrap_err();
        assert_eq!(
            validation_message(error),
            "parameter_filters must be a JSON array"
        );
    }

    #[test]
    fn too_many_conditions_are_rejected() {
        let raw = format!(
            "[{}]",
            (0..=MAX_FILTERS)
                .map(|_| format!(r#"{{"parameter_type_id":"{}"}}"#, Uuid::new_v4()))
                .collect::<Vec<_>>()
                .join(",")
        );
        let error = parse_filter_inputs(Some(&raw)).unwrap_err();
        assert_eq!(
            validation_message(error),
            "parameter_filters cannot contain more than 20 conditions"
        );
    }

    #[test]
    fn duplicate_parameter_type_ids_are_rejected() {
        let parameter_type_id = Uuid::new_v4();
        let raw = format!(
            r#"[{{"parameter_type_id":"{parameter_type_id}"}},{{"parameter_type_id":"{parameter_type_id}"}}]"#
        );
        let error = parse_filter_inputs(Some(&raw)).unwrap_err();
        assert_eq!(
            validation_message(error),
            "parameter_filters cannot contain duplicate parameter_type_id values"
        );
    }

    #[test]
    fn only_numeric_filters_require_units() {
        let millimetre = unit("length", 0.001);
        let mut number = definition("number");
        number.unit_dimension = Some("length".into());
        number.default_unit_id = Some(millimetre.unit_id);
        let mut text = definition("text");
        text.default_unit_id = Some(Uuid::new_v4());

        let definitions = HashMap::from([
            (number.parameter_type_id, number.clone()),
            (text.parameter_type_id, text.clone()),
        ]);
        let inputs = vec![input(&number), input(&text)];

        assert_eq!(
            required_unit_ids(&inputs, &definitions),
            vec![millimetre.unit_id]
        );
    }

    #[test]
    fn text_filter_requires_non_blank_text() {
        let text = definition("text");
        let mut blank = input(&text);
        blank.text = Some("   ".into());

        let error = normalize_filter(blank, &text, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Text parameter filters require text"
        );

        let mut padded = input(&text);
        padded.text = Some("  oscilloscope  ".into());
        let filter = normalize_filter(padded, &text, &HashMap::new()).unwrap();
        assert!(matches!(filter, ParameterFilter::Text { text, .. } if text == "oscilloscope"));
    }

    #[test]
    fn number_filter_requires_a_bound() {
        let number = definition("number");
        let error = normalize_filter(input(&number), &number, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Number parameter filters require number_min or number_max"
        );
    }

    #[test]
    fn number_filter_rejects_inverted_bounds() {
        let number = definition("number");
        let mut inverted = input(&number);
        inverted.number_min = Some(10.0);
        inverted.number_max = Some(1.0);

        let error = normalize_filter(inverted, &number, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "number_min cannot exceed number_max"
        );
    }

    #[test]
    fn unitless_number_filter_compares_raw_values() {
        let number = definition("number");
        let mut bounded = input(&number);
        bounded.number_min = Some(1.5);

        let filter = normalize_filter(bounded, &number, &HashMap::new()).unwrap();
        match filter {
            ParameterFilter::Number {
                min,
                max,
                compare_base,
                ..
            } => {
                assert_eq!(min, Some(1.5));
                assert_eq!(max, None);
                assert!(!compare_base);
            }
            _ => panic!("expected a number filter"),
        }
    }

    #[test]
    fn number_filter_scales_bounds_to_the_base_unit() {
        let millimetre = unit("length", 0.001);
        let mut number = definition("number");
        number.unit_dimension = Some("length".into());
        let mut bounded = input(&number);
        bounded.unit_id = Some(millimetre.unit_id);
        bounded.number_min = Some(2.0);
        bounded.number_max = Some(5.0);

        let filter = normalize_filter(bounded, &number, &units(vec![millimetre])).unwrap();
        match filter {
            ParameterFilter::Number {
                min,
                max,
                compare_base,
                ..
            } => {
                assert_eq!(min, Some(0.002));
                assert_eq!(max, Some(0.005));
                assert!(compare_base);
            }
            _ => panic!("expected a number filter"),
        }
    }

    #[test]
    fn number_filter_falls_back_to_the_default_unit() {
        let millimetre = unit("length", 0.001);
        let mut number = definition("number");
        number.unit_dimension = Some("length".into());
        number.default_unit_id = Some(millimetre.unit_id);
        let mut bounded = input(&number);
        bounded.number_max = Some(3.0);

        let filter = normalize_filter(bounded, &number, &units(vec![millimetre])).unwrap();
        assert!(matches!(
            filter,
            ParameterFilter::Number { max: Some(max), compare_base: true, .. } if max == 0.003
        ));
    }

    #[test]
    fn unit_based_number_filter_requires_a_unit() {
        let mut number = definition("number");
        number.unit_dimension = Some("length".into());
        let mut bounded = input(&number);
        bounded.number_min = Some(1.0);

        let error = normalize_filter(bounded, &number, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Unit-based parameter filters require a unit_id or default unit"
        );
    }

    #[test]
    fn missing_unit_is_rejected() {
        let mut number = definition("number");
        number.unit_dimension = Some("length".into());
        let mut bounded = input(&number);
        bounded.unit_id = Some(Uuid::new_v4());
        bounded.number_min = Some(1.0);

        let error = normalize_filter(bounded, &number, &HashMap::new()).unwrap_err();
        assert_eq!(validation_message(error), "Unit not found");
    }

    #[test]
    fn mismatched_unit_dimension_is_rejected() {
        let gram = unit("mass", 0.001);
        let mut number = definition("number");
        number.unit_dimension = Some("length".into());
        let mut bounded = input(&number);
        bounded.unit_id = Some(gram.unit_id);
        bounded.number_min = Some(1.0);

        let error = normalize_filter(bounded, &number, &units(vec![gram])).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Parameter filter unit dimension does not match parameter definition"
        );
    }

    #[test]
    fn unitless_definition_rejects_an_explicit_unit() {
        let gram = unit("mass", 0.001);
        let number = definition("number");
        let mut bounded = input(&number);
        bounded.unit_id = Some(gram.unit_id);
        bounded.number_min = Some(1.0);

        let error = normalize_filter(bounded, &number, &units(vec![gram])).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Parameter filter unit dimension does not match parameter definition"
        );
    }

    #[test]
    fn range_filter_requires_both_ends() {
        let range = definition("range");
        let mut start_only = input(&range);
        start_only.range_start = Some(1.0);
        let error = normalize_filter(start_only, &range, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Range parameter filters require range_end"
        );

        let mut end_only = input(&range);
        end_only.range_end = Some(1.0);
        let error = normalize_filter(end_only, &range, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Range parameter filters require range_start"
        );
    }

    #[test]
    fn range_filter_rejects_inverted_bounds() {
        let range = definition("range");
        let mut inverted = input(&range);
        inverted.range_start = Some(5.0);
        inverted.range_end = Some(1.0);

        let error = normalize_filter(inverted, &range, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "range_start cannot exceed range_end"
        );
    }

    #[test]
    fn range_filter_scales_both_ends() {
        let kilohertz = unit("frequency", 1000.0);
        let mut range = definition("range");
        range.unit_dimension = Some("frequency".into());
        let mut bounded = input(&range);
        bounded.unit_id = Some(kilohertz.unit_id);
        bounded.range_start = Some(1.0);
        bounded.range_end = Some(2.0);

        let filter = normalize_filter(bounded, &range, &units(vec![kilohertz])).unwrap();
        match filter {
            ParameterFilter::Range {
                start,
                end,
                compare_base,
                ..
            } => {
                assert_eq!(start, 1000.0);
                assert_eq!(end, 2000.0);
                assert!(compare_base);
            }
            _ => panic!("expected a range filter"),
        }
    }

    #[test]
    fn boolean_filter_requires_a_value() {
        let boolean = definition("boolean");
        let error = normalize_filter(input(&boolean), &boolean, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Boolean parameter filters require boolean"
        );

        let mut valued = input(&boolean);
        valued.boolean = Some(true);
        let filter = normalize_filter(valued, &boolean, &HashMap::new()).unwrap();
        assert!(matches!(
            filter,
            ParameterFilter::Boolean { value: true, .. }
        ));
    }

    #[test]
    fn date_filter_requires_a_bound() {
        let date = definition("date");
        let error = normalize_filter(input(&date), &date, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Date parameter filters require date_start or date_end"
        );
    }

    #[test]
    fn date_filter_rejects_inverted_bounds() {
        let date = definition("date");
        let mut inverted = input(&date);
        inverted.date_start = NaiveDate::from_ymd_opt(2026, 8, 10);
        inverted.date_end = NaiveDate::from_ymd_opt(2026, 8, 1);

        let error = normalize_filter(inverted, &date, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "date_start cannot be after date_end"
        );
    }

    #[test]
    fn enum_filter_requires_an_option() {
        let enumeration = definition("enum");
        let error =
            normalize_filter(input(&enumeration), &enumeration, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Enum parameter filters require option_id"
        );

        let option_id = Uuid::new_v4();
        let mut selected = input(&enumeration);
        selected.option_id = Some(option_id);
        let filter = normalize_filter(selected, &enumeration, &HashMap::new()).unwrap();
        assert!(matches!(
            filter,
            ParameterFilter::Enum { option_id: selected, .. } if selected == option_id
        ));
    }

    #[test]
    fn unsupported_data_type_is_rejected() {
        let unsupported = definition("polynomial");
        let error =
            normalize_filter(input(&unsupported), &unsupported, &HashMap::new()).unwrap_err();
        assert_eq!(
            validation_message(error),
            "Invalid asset parameter data type"
        );
    }
}
