use super::model::{AssetParameterResponse, update_asset_parameter_rollback_details};
use super::queries::{
    AssetParameterDatabaseError, fetch_asset_parameter_for_update,
    fetch_asset_parameter_options_for_update, update_asset_parameter_in_database,
};
use super::service::{
    apply_option_updates, normalize_unit_configuration, validate_updated_options,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetParameterCode, AssetParameterDataType, AssetParameterId, AssetParameterName,
    AssetParameterOptionLabel, NullableUpdate, UnitDimension, UpdateAssetParameter,
    UpdateAssetParameterOption, UserId,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;
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
    options: Option<Vec<OptionJsonData>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionJsonData {
    option_id: Option<Uuid>,
    code: String,
    label: String,
    sort_order: Option<i32>,
}

impl TryFrom<OptionJsonData> for UpdateAssetParameterOption {
    type Error = String;

    fn try_from(value: OptionJsonData) -> Result<Self, Self::Error> {
        Ok(Self {
            option_id: value.option_id,
            code: AssetParameterCode::parse(value.code)?,
            label: AssetParameterOptionLabel::parse(value.label)?,
            sort_order: value.sort_order.unwrap_or(0),
        })
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

        Ok(Self {
            code: value.code.map(AssetParameterCode::parse).transpose()?,
            name: value.name.map(AssetParameterName::parse).transpose()?,
            data_type: value
                .data_type
                .as_deref()
                .map(AssetParameterDataType::parse)
                .transpose()?,
            unit_dimension,
            default_unit_id,
            description,
            options,
        })
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

impl From<AssetParameterDatabaseError> for UpdateAssetParameterError {
    fn from(error: AssetParameterDatabaseError) -> Self {
        match error {
            AssetParameterDatabaseError::Validation(message) => Self::ValidationError(message),
            AssetParameterDatabaseError::Conflict(message) => Self::ConflictError(message),
            AssetParameterDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
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
    parameter_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateAssetParameterError> {
    let parameter_id: AssetParameterId = parameter_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::AssetParameter,
        Action::Update(parameter_id.into()),
    )
    .await?
    {
        return Err(UpdateAssetParameterError::Forbidden(
            "You don't have permission to update this asset parameter.".into(),
        ));
    }
    let update_parameter = UpdateAssetParameter::try_from(payload.into_inner())
        .map_err(UpdateAssetParameterError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_asset_parameter_for_update(&mut transaction, parameter_id)
        .await?
        .ok_or(UpdateAssetParameterError::NotFound(
            "Asset parameter not found".into(),
        ))?;
    let existing_options =
        fetch_asset_parameter_options_for_update(&mut transaction, existing.parameter_type_id)
            .await?;

    let data_type = update_parameter.data_type.unwrap_or(
        AssetParameterDataType::parse(&existing.data_type).map_err(|e| {
            UpdateAssetParameterError::UnexpectedError(anyhow!("Invalid stored data type: {e}"))
        })?,
    );
    validate_updated_options(
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
    let updated = update_asset_parameter_in_database(
        &mut transaction,
        existing.parameter_type_id,
        &code,
        &name,
        data_type,
        unit_dimension.as_deref(),
        default_unit_id,
        description.as_deref(),
    )
    .await?;
    let options = apply_option_updates(
        &mut transaction,
        updated.parameter_type_id,
        data_type,
        &existing_options,
        update_parameter.options.as_deref(),
    )
    .await?;

    record_audit(
        &mut transaction,
        actor_user_id,
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
