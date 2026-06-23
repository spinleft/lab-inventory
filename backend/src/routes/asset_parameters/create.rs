use super::model::{
    AssetParameterDatabaseError, AssetParameterOptionRow, AssetParameterResponse,
    AssetParameterRow, create_asset_parameter_rollback_details, fetch_unit_dimension_for_update,
    map_database_error,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetParameterCode, AssetParameterDataType, AssetParameterName, AssetParameterOptionLabel,
    LaboratoryId, NewAssetParameter, NewAssetParameterOption, UnitDimension, UserId,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    code: String,
    name: String,
    data_type: String,
    unit_dimension: Option<String>,
    default_unit_id: Option<Uuid>,
    description: Option<String>,
    is_archived: Option<bool>,
    options: Option<Vec<OptionJsonData>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionJsonData {
    code: String,
    label: String,
    sort_order: Option<i32>,
    is_archived: Option<bool>,
}

impl TryFrom<OptionJsonData> for NewAssetParameterOption {
    type Error = String;

    fn try_from(value: OptionJsonData) -> Result<Self, Self::Error> {
        Ok(Self::new(
            AssetParameterCode::parse(value.code)?,
            AssetParameterOptionLabel::parse(value.label)?,
            value.sort_order.unwrap_or(0),
            value.is_archived.unwrap_or(false),
        ))
    }
}

impl TryFrom<JsonData> for NewAssetParameter {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let options = value
            .options
            .unwrap_or_default()
            .into_iter()
            .map(NewAssetParameterOption::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self::new(
            AssetParameterCode::parse(value.code)?,
            AssetParameterName::parse(value.name)?,
            AssetParameterDataType::parse(&value.data_type)?,
            value
                .unit_dimension
                .as_deref()
                .map(UnitDimension::parse)
                .transpose()?,
            value.default_unit_id,
            value.description,
            value.is_archived.unwrap_or(false),
            options,
        ))
    }
}

#[derive(thiserror::Error)]
pub enum CreateAssetParameterError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreateAssetParameterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateAssetParameterError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateAssetParameterError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateAssetParameterError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateAssetParameterError::ConflictError(_) => StatusCode::CONFLICT,
            CreateAssetParameterError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Create an asset parameter",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn create_asset_parameter(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateAssetParameterError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id.into_inner())
        .map_err(CreateAssetParameterError::ValidationError)?;
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(CreateAssetParameterError::UnexpectedError)?
        .ok_or(CreateAssetParameterError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_create_permission(&actor, &laboratory_id)?;

    let new_parameter = NewAssetParameter::try_from(payload.into_inner())
        .map_err(CreateAssetParameterError::ValidationError)?;
    validate_options(new_parameter.data_type, &new_parameter.options)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let unit_dimension = normalize_unit_configuration(
        &mut transaction,
        new_parameter.data_type,
        new_parameter
            .unit_dimension
            .as_ref()
            .map(|dimension| dimension.as_ref()),
        new_parameter.default_unit_id,
    )
    .await?;
    let parameter = insert_asset_parameter(
        &mut transaction,
        laboratory_id,
        new_parameter.code.as_ref(),
        new_parameter.name.as_ref(),
        new_parameter.data_type,
        unit_dimension.as_deref(),
        new_parameter.default_unit_id,
        new_parameter.description.as_deref(),
        new_parameter.is_archived,
    )
    .await?;
    let options = insert_asset_parameter_options(
        &mut transaction,
        parameter.parameter_type_id,
        &new_parameter.options,
    )
    .await?;

    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Create,
        AuditResource::AssetParameter,
        Some(parameter.parameter_type_id),
        create_asset_parameter_rollback_details(&parameter),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new asset parameter.")?;

    Ok(HttpResponse::Created().json(AssetParameterResponse::from_parts(parameter, options)))
}

fn validate_create_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), CreateAssetParameterError> {
    if actor.can_write_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(CreateAssetParameterError::Forbidden(
            "You don't have permission to create asset parameters for this laboratory.".into(),
        ))
    }
}

fn validate_options(
    data_type: AssetParameterDataType,
    options: &[NewAssetParameterOption],
) -> Result<(), CreateAssetParameterError> {
    if data_type != AssetParameterDataType::Enum && !options.is_empty() {
        return Err(CreateAssetParameterError::ValidationError(
            "Options are only allowed for enum asset parameters".into(),
        ));
    }
    if data_type == AssetParameterDataType::Enum
        && !options.iter().any(|option| !option.is_archived)
    {
        return Err(CreateAssetParameterError::ValidationError(
            "Enum asset parameters require at least one active option".into(),
        ));
    }

    let mut seen_codes = HashSet::new();
    for option in options {
        if !seen_codes.insert(option.code.as_ref().to_string()) {
            return Err(CreateAssetParameterError::ValidationError(
                "Option codes must be unique".into(),
            ));
        }
    }

    Ok(())
}

async fn normalize_unit_configuration(
    transaction: &mut Transaction<'_, Postgres>,
    data_type: AssetParameterDataType,
    unit_dimension: Option<&str>,
    default_unit_id: Option<Uuid>,
) -> Result<Option<String>, CreateAssetParameterError> {
    if !matches!(
        data_type,
        AssetParameterDataType::Number | AssetParameterDataType::Range
    ) {
        if unit_dimension.is_some() || default_unit_id.is_some() {
            return Err(CreateAssetParameterError::ValidationError(
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
        .ok_or(CreateAssetParameterError::ValidationError(
            "Default unit not found".into(),
        ))?;

    if let Some(unit_dimension) = unit_dimension {
        if unit_dimension != default_unit_dimension {
            return Err(CreateAssetParameterError::ValidationError(
                "Default unit dimension does not match asset parameter unit dimension".into(),
            ));
        }
    }

    Ok(Some(default_unit_dimension))
}

#[tracing::instrument(
    name = "Saving new asset parameter in the database",
    skip(transaction, code, name, unit_dimension, description),
    fields(laboratory_id=%laboratory_id)
)]
async fn insert_asset_parameter(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    code: &str,
    name: &str,
    data_type: AssetParameterDataType,
    unit_dimension: Option<&str>,
    default_unit_id: Option<Uuid>,
    description: Option<&str>,
    is_archived: bool,
) -> Result<AssetParameterRow, CreateAssetParameterError> {
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
            description,
            is_archived
        )
        VALUES ($1, $2, $3, $4, $5::asset_parameter_data_type, $6, $7, $8, $9)
        RETURNING
            parameter_type_id,
            laboratory_id,
            code,
            name,
            data_type::text AS data_type,
            unit_dimension,
            default_unit_id,
            description,
            is_archived,
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
    .bind(is_archived)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_error)
}

async fn insert_asset_parameter_options(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    options: &[NewAssetParameterOption],
) -> Result<Vec<AssetParameterOptionRow>, CreateAssetParameterError> {
    let mut rows = Vec::with_capacity(options.len());
    for option in options {
        let row = sqlx::query_as::<_, AssetParameterOptionRow>(
            r#"
            INSERT INTO asset_parameter_options (
                option_id,
                parameter_type_id,
                code,
                label,
                sort_order,
                is_archived
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING option_id, parameter_type_id, code, label, sort_order, is_archived
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(parameter_id)
        .bind(option.code.as_ref())
        .bind(option.label.as_ref())
        .bind(option.sort_order)
        .bind(option.is_archived)
        .fetch_one(transaction.as_mut())
        .await
        .map_err(map_error)?;
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

fn map_error(error: sqlx::Error) -> CreateAssetParameterError {
    if let Some(mapped) = map_database_error(
        &error,
        "Asset parameter code already exists in this laboratory",
        "Asset parameter option code already exists",
        "Asset parameter already exists",
    ) {
        return match mapped {
            AssetParameterDatabaseError::Conflict(message) => {
                CreateAssetParameterError::ConflictError(message)
            }
            AssetParameterDatabaseError::Validation(message) => {
                CreateAssetParameterError::ValidationError(message)
            }
        };
    }

    CreateAssetParameterError::UnexpectedError(error.into())
}
