use crate::domain::{AssetParameterId, LaboratoryId};
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Serialize)]
pub(super) struct AssetParameterResponse {
    parameter_type_id: Uuid,
    laboratory_id: Uuid,
    code: String,
    name: String,
    data_type: String,
    unit_dimension: Option<String>,
    default_unit_id: Option<Uuid>,
    description: Option<String>,
    options: Vec<AssetParameterOptionResponse>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetParameterRow {
    pub(super) parameter_type_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) code: String,
    pub(super) name: String,
    pub(super) data_type: String,
    pub(super) unit_dimension: Option<String>,
    pub(super) default_unit_id: Option<Uuid>,
    pub(super) description: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub(super) struct AssetParameterOptionResponse {
    option_id: Uuid,
    parameter_type_id: Uuid,
    code: String,
    label: String,
    sort_order: i32,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(super) struct AssetParameterOptionRow {
    pub(super) option_id: Uuid,
    pub(super) parameter_type_id: Uuid,
    pub(super) code: String,
    pub(super) label: String,
    pub(super) sort_order: i32,
}

impl From<AssetParameterOptionRow> for AssetParameterOptionResponse {
    fn from(row: AssetParameterOptionRow) -> Self {
        Self {
            option_id: row.option_id,
            parameter_type_id: row.parameter_type_id,
            code: row.code,
            label: row.label,
            sort_order: row.sort_order,
        }
    }
}

impl AssetParameterResponse {
    pub(super) fn from_parts(
        row: AssetParameterRow,
        options: Vec<AssetParameterOptionRow>,
    ) -> Self {
        Self {
            parameter_type_id: row.parameter_type_id,
            laboratory_id: row.laboratory_id,
            code: row.code,
            name: row.name,
            data_type: row.data_type,
            unit_dimension: row.unit_dimension,
            default_unit_id: row.default_unit_id,
            description: row.description,
            options: options
                .into_iter()
                .map(|o| AssetParameterOptionResponse {
                    option_id: o.option_id,
                    parameter_type_id: o.parameter_type_id,
                    code: o.code,
                    label: o.label,
                    sort_order: o.sort_order,
                })
                .collect(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(super) fn create_asset_parameter_rollback_details(parameter: &AssetParameterRow) -> Value {
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "asset_parameter",
            "where": {
                "parameter_type_id": parameter.parameter_type_id,
            },
        },
    })
}

pub(super) fn update_asset_parameter_rollback_details(
    parameter: &AssetParameterRow,
    options: &[AssetParameterOptionRow],
) -> Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "asset_parameter",
            "where": {
                "parameter_type_id": parameter.parameter_type_id,
            },
            "values": {
                "laboratory_id": parameter.laboratory_id,
                "code": &parameter.code,
                "name": &parameter.name,
                "data_type": &parameter.data_type,
                "unit_dimension": parameter.unit_dimension.as_deref(),
                "default_unit_id": parameter.default_unit_id,
                "description": parameter.description.as_deref(),
                "options": options,
                "updated_at": parameter.updated_at,
            },
        },
    })
}

pub(super) fn delete_asset_parameter_rollback_details(
    parameter: &AssetParameterRow,
    options: &[AssetParameterOptionRow],
) -> Value {
    json!({
        "rollback": {
            "operation": "create",
            "resource_type": "asset_parameter",
            "values": {
                "parameter": parameter,
                "options": options,
            },
        },
    })
}

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

pub(super) fn map_database_error(
    error: &sqlx::Error,
    duplicate_parameter_code: &str,
    duplicate_option_code: &str,
    generic_unique: &str,
) -> Option<AssetParameterDatabaseError> {
    let sqlx::Error::Database(database_error) = error else {
        return None;
    };

    match (
        database_error.code().as_deref(),
        database_error.constraint(),
    ) {
        (Some("23505"), Some("asset_parameter_types_laboratory_id_code_key")) => Some(
            AssetParameterDatabaseError::Conflict(duplicate_parameter_code.into()),
        ),
        (Some("23505"), Some("asset_parameter_options_parameter_type_id_code_key")) => Some(
            AssetParameterDatabaseError::Conflict(duplicate_option_code.into()),
        ),
        (Some("23505"), _) => Some(AssetParameterDatabaseError::Conflict(generic_unique.into())),
        (Some("23503"), _) => Some(AssetParameterDatabaseError::Validation(
            "Invalid referenced record".into(),
        )),
        (Some("23514"), _) => Some(AssetParameterDatabaseError::Validation(
            "Invalid asset parameter".into(),
        )),
        _ => None,
    }
}

pub(super) fn parse_laboratory_id(laboratory_id: Uuid) -> Result<LaboratoryId, anyhow::Error> {
    LaboratoryId::parse(laboratory_id).map_err(|e| anyhow::anyhow!("{e}"))
}

pub(super) enum AssetParameterDatabaseError {
    Conflict(String),
    Validation(String),
}
