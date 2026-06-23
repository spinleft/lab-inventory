use super::model::{
    AssetParameterDatabaseError, AssetParameterOptionRow, AssetParameterResponse,
    AssetParameterRow, fetch_asset_parameter_for_update, fetch_asset_parameter_options_for_update,
    fetch_unit_dimension_for_update, map_database_error, update_asset_parameter_rollback_details,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetParameterCode, AssetParameterDataType, AssetParameterId, AssetParameterName,
    AssetParameterOptionLabel, LaboratoryId, NullableUpdate, UnitDimension, UpdateAssetParameter,
    UpdateAssetParameterOption, UserId,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    code: Option<String>,
    name: Option<String>,
    data_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    unit_dimension: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    default_unit_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    description: Option<Option<String>>,
    is_archived: Option<bool>,
    options: Option<Vec<OptionJsonData>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionJsonData {
    option_id: Option<Uuid>,
    code: String,
    label: String,
    sort_order: Option<i32>,
    is_archived: Option<bool>,
}

impl TryFrom<OptionJsonData> for UpdateAssetParameterOption {
    type Error = String;

    fn try_from(value: OptionJsonData) -> Result<Self, Self::Error> {
        Ok(Self::new(
            value.option_id,
            AssetParameterCode::parse(value.code)?,
            AssetParameterOptionLabel::parse(value.label)?,
            value.sort_order.unwrap_or(0),
            value.is_archived.unwrap_or(false),
        ))
    }
}

impl TryFrom<JsonData> for UpdateAssetParameter {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let unit_dimension = match value.unit_dimension {
            Some(Some(unit_dimension)) => {
                UnitDimension::parse(&unit_dimension).map(NullableUpdate::Set)?
            }
            Some(None) => NullableUpdate::Clear,
            None => NullableUpdate::Unchanged,
        };
        let default_unit_id = match value.default_unit_id {
            Some(Some(default_unit_id)) => NullableUpdate::Set(default_unit_id),
            Some(None) => NullableUpdate::Clear,
            None => NullableUpdate::Unchanged,
        };
        let description = match value.description {
            Some(Some(description)) => NullableUpdate::Set(description),
            Some(None) => NullableUpdate::Clear,
            None => NullableUpdate::Unchanged,
        };
        let options = value
            .options
            .map(|options| {
                options
                    .into_iter()
                    .map(UpdateAssetParameterOption::try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        Ok(Self::new(
            value.code.map(AssetParameterCode::parse).transpose()?,
            value.name.map(AssetParameterName::parse).transpose()?,
            value
                .data_type
                .as_deref()
                .map(AssetParameterDataType::parse)
                .transpose()?,
            unit_dimension,
            default_unit_id,
            description,
            value.is_archived,
            options,
        ))
    }
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(thiserror::Error)]
pub enum UpdateAssetParameterError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for UpdateAssetParameterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UpdateAssetParameterError {
    fn status_code(&self) -> StatusCode {
        match self {
            UpdateAssetParameterError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UpdateAssetParameterError::Forbidden(_) => StatusCode::FORBIDDEN,
            UpdateAssetParameterError::NotFound(_) => StatusCode::NOT_FOUND,
            UpdateAssetParameterError::ConflictError(_) => StatusCode::CONFLICT,
            UpdateAssetParameterError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Update an asset parameter",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, parameter_id=%parameter_id)
)]
pub async fn update_asset_parameter(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    parameter_id: web::Path<AssetParameterId>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateAssetParameterError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(UpdateAssetParameterError::UnexpectedError)?
        .ok_or(UpdateAssetParameterError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    let update_parameter = UpdateAssetParameter::try_from(payload.into_inner())
        .map_err(UpdateAssetParameterError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_asset_parameter_for_update(&mut transaction, *parameter_id)
        .await?
        .ok_or(UpdateAssetParameterError::NotFound(
            "Asset parameter not found".into(),
        ))?;
    let existing_options =
        fetch_asset_parameter_options_for_update(&mut transaction, existing.parameter_type_id)
            .await?;
    let laboratory_id = LaboratoryId::parse(existing.laboratory_id)
        .map_err(|e| UpdateAssetParameterError::UnexpectedError(anyhow!("{e}")))?;
    validate_update_permission(&actor, &laboratory_id)?;

    let data_type = update_parameter.data_type.unwrap_or(
        AssetParameterDataType::parse(&existing.data_type).map_err(|e| {
            UpdateAssetParameterError::UnexpectedError(anyhow!("Invalid stored data type: {e}"))
        })?,
    );
    validate_options(
        data_type,
        update_parameter.options.as_deref(),
        &existing_options,
    )?;

    let code = update_parameter
        .code
        .as_ref()
        .map(|code| code.as_ref())
        .unwrap_or(&existing.code)
        .to_string();
    let name = update_parameter
        .name
        .as_ref()
        .map(|name| name.as_ref())
        .unwrap_or(&existing.name)
        .to_string();
    let current_unit_dimension = existing
        .unit_dimension
        .as_deref()
        .map(UnitDimension::parse)
        .transpose()
        .map_err(UpdateAssetParameterError::ValidationError)?;
    let unit_dimension = update_parameter
        .unit_dimension
        .resolve(current_unit_dimension);
    let default_unit_id = update_parameter
        .default_unit_id
        .resolve(existing.default_unit_id);
    let unit_dimension = normalize_unit_configuration(
        &mut transaction,
        data_type,
        unit_dimension.as_ref().map(|dimension| dimension.as_ref()),
        default_unit_id,
    )
    .await?;
    let description = update_parameter
        .description
        .resolve(existing.description.clone());
    let is_archived = update_parameter.is_archived.unwrap_or(existing.is_archived);

    let updated = update_asset_parameter_in_database(
        &mut transaction,
        existing.parameter_type_id,
        &code,
        &name,
        data_type,
        unit_dimension.as_deref(),
        default_unit_id,
        description.as_deref(),
        is_archived,
    )
    .await?;
    let options = if data_type == AssetParameterDataType::Enum {
        match update_parameter.options {
            Some(options) => {
                replace_asset_parameter_options(
                    &mut transaction,
                    updated.parameter_type_id,
                    &existing_options,
                    &options,
                )
                .await?
            }
            None => existing_options.clone(),
        }
    } else {
        delete_asset_parameter_options(&mut transaction, updated.parameter_type_id).await?;
        Vec::new()
    };

    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Update,
        AuditResource::AssetParameter,
        Some(updated.parameter_type_id),
        update_asset_parameter_rollback_details(&existing, &existing_options),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update an asset parameter.")?;

    Ok(HttpResponse::Ok().json(AssetParameterResponse::from_parts(updated, options)))
}

fn validate_update_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), UpdateAssetParameterError> {
    if actor.can_write_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(UpdateAssetParameterError::Forbidden(
            "You don't have permission to update asset parameters for this laboratory.".into(),
        ))
    }
}

fn validate_options(
    data_type: AssetParameterDataType,
    update_options: Option<&[UpdateAssetParameterOption]>,
    existing_options: &[AssetParameterOptionRow],
) -> Result<(), UpdateAssetParameterError> {
    if data_type != AssetParameterDataType::Enum {
        if update_options.is_some_and(|options| !options.is_empty()) {
            return Err(UpdateAssetParameterError::ValidationError(
                "Options are only allowed for enum asset parameters".into(),
            ));
        }
        return Ok(());
    }

    match update_options {
        Some(options) => validate_update_options(options),
        None => {
            if existing_options.iter().any(|option| !option.is_archived) {
                Ok(())
            } else {
                Err(UpdateAssetParameterError::ValidationError(
                    "Enum asset parameters require at least one active option".into(),
                ))
            }
        }
    }
}

fn validate_update_options(
    options: &[UpdateAssetParameterOption],
) -> Result<(), UpdateAssetParameterError> {
    if !options.iter().any(|option| !option.is_archived) {
        return Err(UpdateAssetParameterError::ValidationError(
            "Enum asset parameters require at least one active option".into(),
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut seen_codes = HashSet::new();
    for option in options {
        if let Some(option_id) = option.option_id {
            if !seen_ids.insert(option_id) {
                return Err(UpdateAssetParameterError::ValidationError(
                    "Option ids must be unique".into(),
                ));
            }
        }
        if !seen_codes.insert(option.code.as_ref().to_string()) {
            return Err(UpdateAssetParameterError::ValidationError(
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
) -> Result<Option<String>, UpdateAssetParameterError> {
    if data_type != AssetParameterDataType::Number {
        if unit_dimension.is_some() || default_unit_id.is_some() {
            return Err(UpdateAssetParameterError::ValidationError(
                "Units are only allowed for number asset parameters".into(),
            ));
        }
        return Ok(None);
    }

    let Some(default_unit_id) = default_unit_id else {
        return Ok(unit_dimension.map(ToOwned::to_owned));
    };

    let default_unit_dimension = fetch_unit_dimension_for_update(transaction, default_unit_id)
        .await?
        .ok_or(UpdateAssetParameterError::ValidationError(
            "Default unit not found".into(),
        ))?;

    if let Some(unit_dimension) = unit_dimension {
        if unit_dimension != default_unit_dimension {
            return Err(UpdateAssetParameterError::ValidationError(
                "Default unit dimension does not match asset parameter unit dimension".into(),
            ));
        }
    }

    Ok(Some(default_unit_dimension))
}

#[tracing::instrument(
    name = "Updating asset parameter in the database",
    skip(transaction, code, name, unit_dimension, description),
    fields(parameter_id=%parameter_id)
)]
async fn update_asset_parameter_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    code: &str,
    name: &str,
    data_type: AssetParameterDataType,
    unit_dimension: Option<&str>,
    default_unit_id: Option<Uuid>,
    description: Option<&str>,
    is_archived: bool,
) -> Result<AssetParameterRow, UpdateAssetParameterError> {
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
            is_archived = $8,
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
            is_archived,
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
    .bind(is_archived)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_error)
}

async fn replace_asset_parameter_options(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    existing_options: &[AssetParameterOptionRow],
    options: &[UpdateAssetParameterOption],
) -> Result<Vec<AssetParameterOptionRow>, UpdateAssetParameterError> {
    let existing_by_id: HashMap<Uuid, &AssetParameterOptionRow> = existing_options
        .iter()
        .map(|option| (option.option_id, option))
        .collect();
    let existing_by_code: HashMap<&str, &AssetParameterOptionRow> = existing_options
        .iter()
        .map(|option| (option.code.as_str(), option))
        .collect();

    for option in options {
        if let Some(option_id) = option.option_id {
            if !existing_by_id.contains_key(&option_id) {
                return Err(UpdateAssetParameterError::ValidationError(
                    "Asset parameter option not found".into(),
                ));
            }
        }
    }

    archive_asset_parameter_options(transaction, parameter_id).await?;
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
        .map_err(UpdateAssetParameterError::UnexpectedError)
}

async fn archive_asset_parameter_options(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
) -> Result<(), UpdateAssetParameterError> {
    sqlx::query(
        r#"
        UPDATE asset_parameter_options
        SET is_archived = true
        WHERE parameter_type_id = $1
        "#,
    )
    .bind(parameter_id)
    .execute(transaction.as_mut())
    .await
    .map_err(map_error)?;

    Ok(())
}

async fn update_asset_parameter_option(
    transaction: &mut Transaction<'_, Postgres>,
    option_id: Uuid,
    option: &UpdateAssetParameterOption,
) -> Result<(), UpdateAssetParameterError> {
    sqlx::query(
        r#"
        UPDATE asset_parameter_options
        SET
            code = $2,
            label = $3,
            sort_order = $4,
            is_archived = $5
        WHERE option_id = $1
        "#,
    )
    .bind(option_id)
    .bind(option.code.as_ref())
    .bind(option.label.as_ref())
    .bind(option.sort_order)
    .bind(option.is_archived)
    .execute(transaction.as_mut())
    .await
    .map_err(map_error)?;

    Ok(())
}

async fn insert_asset_parameter_option(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
    option: &UpdateAssetParameterOption,
) -> Result<(), UpdateAssetParameterError> {
    sqlx::query(
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
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(parameter_id)
    .bind(option.code.as_ref())
    .bind(option.label.as_ref())
    .bind(option.sort_order)
    .bind(option.is_archived)
    .execute(transaction.as_mut())
    .await
    .map_err(map_error)?;

    Ok(())
}

async fn delete_asset_parameter_options(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
) -> Result<(), UpdateAssetParameterError> {
    sqlx::query("DELETE FROM asset_parameter_options WHERE parameter_type_id = $1")
        .bind(parameter_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_error)?;

    Ok(())
}

fn map_error(error: sqlx::Error) -> UpdateAssetParameterError {
    if let Some(mapped) = map_database_error(
        &error,
        "Asset parameter code already exists in this laboratory",
        "Asset parameter option code already exists",
        "Asset parameter already exists",
    ) {
        return match mapped {
            AssetParameterDatabaseError::Conflict(message) => {
                UpdateAssetParameterError::ConflictError(message)
            }
            AssetParameterDatabaseError::Validation(message) => {
                UpdateAssetParameterError::ValidationError(message)
            }
        };
    }

    UpdateAssetParameterError::UnexpectedError(error.into())
}
