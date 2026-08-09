use crate::domain::{AssetParameterDataType, UnitId};
use chrono::NaiveDate;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub enum AssetParameterValue {
    Text(String),
    Number {
        number: f64,
        unit_id: Option<UnitId>,
    },
    Range {
        start: f64,
        end: f64,
        unit_id: Option<UnitId>,
    },
    Boolean(bool),
    Date(NaiveDate),
    Enum(Uuid),
}

impl AssetParameterValue {
    pub fn parse(data_type: AssetParameterDataType, value: &Value) -> Result<Self, String> {
        match data_type {
            AssetParameterDataType::Text => Ok(Self::Text(parse_text(value)?)),
            AssetParameterDataType::Number => {
                let (number, unit_id) = parse_number(value)?;
                Ok(Self::Number { number, unit_id })
            }
            AssetParameterDataType::Range => {
                let (start, end, unit_id) = parse_range(value)?;
                if start > end {
                    return Err("range_start cannot exceed range_end".into());
                }
                Ok(Self::Range {
                    start,
                    end,
                    unit_id,
                })
            }
            AssetParameterDataType::Boolean => Ok(Self::Boolean(parse_boolean(value)?)),
            AssetParameterDataType::Date => Ok(Self::Date(parse_date(value)?)),
            AssetParameterDataType::Enum => Ok(Self::Enum(parse_option(value)?)),
        }
    }
}

fn parse_text(value: &Value) -> Result<String, String> {
    if let Some(text) = value.as_str() {
        return Ok(text.to_string());
    }
    value
        .get("text")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or("Text parameter value must be a string".into())
}

fn parse_number(value: &Value) -> Result<(f64, Option<UnitId>), String> {
    if let Some(number) = value.as_f64() {
        return Ok((number, None));
    }
    let number = value
        .get("number")
        .and_then(Value::as_f64)
        .ok_or("Number parameter value must include number")?;

    Ok((number, parse_unit_id(value)?))
}

fn parse_range(value: &Value) -> Result<(f64, f64, Option<UnitId>), String> {
    let start = value
        .get("range_start")
        .or_else(|| value.get("start"))
        .and_then(Value::as_f64)
        .ok_or("Range parameter value must include range_start")?;
    let end = value
        .get("range_end")
        .or_else(|| value.get("end"))
        .and_then(Value::as_f64)
        .ok_or("Range parameter value must include range_end")?;

    Ok((start, end, parse_unit_id(value)?))
}

fn parse_boolean(value: &Value) -> Result<bool, String> {
    if let Some(boolean) = value.as_bool() {
        return Ok(boolean);
    }
    value
        .get("boolean")
        .and_then(Value::as_bool)
        .ok_or("Boolean parameter value must be a boolean".into())
}

fn parse_date(value: &Value) -> Result<NaiveDate, String> {
    let date = value
        .as_str()
        .or_else(|| value.get("date").and_then(Value::as_str))
        .ok_or("Date parameter value must be an ISO date string")?;

    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "Invalid date parameter value".to_string())
}

fn parse_option(value: &Value) -> Result<Uuid, String> {
    if let Some(option_id) = value.as_str() {
        return Uuid::parse_str(option_id).map_err(|_| "Invalid enum option id".to_string());
    }

    parse_uuid_field(value, "option_id")?.ok_or("Enum parameter value requires option_id".into())
}

fn parse_unit_id(value: &Value) -> Result<Option<UnitId>, String> {
    Ok(parse_uuid_field(value, "unit_id")?.map(UnitId))
}

fn parse_uuid_field(value: &Value, field: &str) -> Result<Option<Uuid>, String> {
    value
        .get(field)
        .map(|value| {
            value
                .as_str()
                .ok_or(format!("{field} must be a uuid string"))
        })
        .transpose()?
        .map(|value| Uuid::parse_str(value).map_err(|_| format!("{field} must be a uuid string")))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::AssetParameterValue;
    use crate::domain::AssetParameterDataType;
    use claims::{assert_err, assert_ok};
    use serde_json::json;

    #[test]
    fn text_values_are_parsed_from_a_bare_string_or_a_wrapper_object() {
        assert_eq!(
            AssetParameterValue::parse(AssetParameterDataType::Text, &json!("hello")).unwrap(),
            AssetParameterValue::Text("hello".into())
        );
        assert_eq!(
            AssetParameterValue::parse(AssetParameterDataType::Text, &json!({ "text": "hello" }))
                .unwrap(),
            AssetParameterValue::Text("hello".into())
        );
    }

    #[test]
    fn text_values_that_are_not_strings_are_rejected() {
        assert_err!(AssetParameterValue::parse(
            AssetParameterDataType::Text,
            &json!(12)
        ));
    }

    #[test]
    fn number_values_accept_an_optional_unit_id() {
        assert_ok!(AssetParameterValue::parse(
            AssetParameterDataType::Number,
            &json!(1.5)
        ));
        assert_ok!(AssetParameterValue::parse(
            AssetParameterDataType::Number,
            &json!({
                "number": 1.5,
                "unit_id": "00000000-0000-0000-0000-000000000001",
            })
        ));
    }

    #[test]
    fn number_values_without_a_number_are_rejected() {
        assert_err!(AssetParameterValue::parse(
            AssetParameterDataType::Number,
            &json!({ "unit_id": "00000000-0000-0000-0000-000000000001" })
        ));
    }

    #[test]
    fn range_values_accept_both_the_long_and_the_short_field_names() {
        assert_ok!(AssetParameterValue::parse(
            AssetParameterDataType::Range,
            &json!({ "range_start": 1.0, "range_end": 2.0 })
        ));
        assert_ok!(AssetParameterValue::parse(
            AssetParameterDataType::Range,
            &json!({ "start": 1.0, "end": 2.0 })
        ));
    }

    #[test]
    fn ranges_with_a_start_greater_than_the_end_are_rejected() {
        assert_err!(AssetParameterValue::parse(
            AssetParameterDataType::Range,
            &json!({ "range_start": 2.0, "range_end": 1.0 })
        ));
    }

    #[test]
    fn boolean_values_are_parsed_from_a_bare_boolean_or_a_wrapper_object() {
        assert_eq!(
            AssetParameterValue::parse(AssetParameterDataType::Boolean, &json!(true)).unwrap(),
            AssetParameterValue::Boolean(true)
        );
        assert_eq!(
            AssetParameterValue::parse(
                AssetParameterDataType::Boolean,
                &json!({ "boolean": false })
            )
            .unwrap(),
            AssetParameterValue::Boolean(false)
        );
    }

    #[test]
    fn date_values_must_use_the_iso_format() {
        assert_ok!(AssetParameterValue::parse(
            AssetParameterDataType::Date,
            &json!("2026-07-26")
        ));
        assert_err!(AssetParameterValue::parse(
            AssetParameterDataType::Date,
            &json!("26/07/2026")
        ));
    }

    #[test]
    fn enum_values_require_a_valid_option_id() {
        assert_ok!(AssetParameterValue::parse(
            AssetParameterDataType::Enum,
            &json!("00000000-0000-0000-0000-000000000001")
        ));
        assert_ok!(AssetParameterValue::parse(
            AssetParameterDataType::Enum,
            &json!({ "option_id": "00000000-0000-0000-0000-000000000001" })
        ));
        assert_err!(AssetParameterValue::parse(
            AssetParameterDataType::Enum,
            &json!({ "option_id": "not-a-uuid" })
        ));
        assert_err!(AssetParameterValue::parse(
            AssetParameterDataType::Enum,
            &json!({})
        ));
    }
}
