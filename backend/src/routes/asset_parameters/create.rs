use super::model::{AssetParameterResponse, create_asset_parameter_rollback_details};
use super::queries::{AssetParameterDatabaseError, insert_asset_parameter};
use super::service::{insert_new_options, normalize_unit_configuration, validate_new_options};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetParameterCode, AssetParameterDataType, AssetParameterName, AssetParameterOptionLabel,
    NewAssetParameter, NewAssetParameterOption, UnitDimension,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
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
    options: Option<Vec<OptionJsonData>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionJsonData {
    code: String,
    label: String,
    sort_order: Option<i32>,
}

impl TryFrom<OptionJsonData> for NewAssetParameterOption {
    type Error = String;

    fn try_from(value: OptionJsonData) -> Result<Self, Self::Error> {
        Ok(Self {
            code: AssetParameterCode::parse(value.code)?,
            label: AssetParameterOptionLabel::parse(value.label)?,
            sort_order: value.sort_order.unwrap_or(0),
        })
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

        Ok(Self {
            code: AssetParameterCode::parse(value.code)?,
            name: AssetParameterName::parse(value.name)?,
            data_type: AssetParameterDataType::parse(&value.data_type)?,
            unit_dimension: value
                .unit_dimension
                .as_deref()
                .map(UnitDimension::parse)
                .transpose()?,
            default_unit_id: value.default_unit_id,
            description: value.description,
            options,
        })
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

impl From<AssetParameterDatabaseError> for CreateAssetParameterError {
    fn from(error: AssetParameterDatabaseError) -> Self {
        match error {
            AssetParameterDatabaseError::Validation(message) => Self::ValidationError(message),
            AssetParameterDatabaseError::Conflict(message) => Self::ConflictError(message),
            AssetParameterDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Create an asset parameter",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn create_asset_parameter(
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateAssetParameterError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    if !validate_permission(
        &pool,
        actor,
        ResourceType::AssetParameter,
        Action::Create(laboratory_id.into()),
    )
    .await?
    {
        return Err(CreateAssetParameterError::Forbidden(
            "You don't have permission to create asset parameters.".into(),
        ));
    }

    let new_parameter = NewAssetParameter::try_from(payload.into_inner())
        .map_err(CreateAssetParameterError::ValidationError)?;
    validate_new_options(new_parameter.data_type, &new_parameter.options)?;

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
    )
    .await?;
    let options = insert_new_options(
        &mut transaction,
        parameter.parameter_type_id,
        &new_parameter.options,
    )
    .await?;

    record_audit(
        &mut transaction,
        actor,
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
