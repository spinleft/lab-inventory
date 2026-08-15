use super::model::{
    AssetParameterValueInput, AssetResponse, AssetRow, update_asset_rollback_details,
};
use super::queries::{
    AssetDatabaseError, fetch_asset_for_update, fetch_inventory_items_for_asset_for_update,
    fetch_parameter_values_for_asset_for_update, update_asset_in_database, validate_category,
};
use super::service::{
    apply_asset_parameter_updates, convert_inventory_quantities_to_unit,
    validate_required_parameters,
};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetCategoryId, AssetId, AssetName, AssetTrackingMode, LaboratoryId, NullableUpdate,
    UpdateAsset,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    #[serde(default, deserialize_with = "deserialize_nullable")]
    category_id: Option<Option<Uuid>>,
    tracking_mode: Option<String>,
    name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    model: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    manufacturer: Option<Option<String>>,
    inventory_unit_id: Option<Uuid>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    public_notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    internal_notes: Option<Option<String>>,
    parameters: Option<Vec<ParameterValueJsonData>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterValueJsonData {
    parameter_type_id: Uuid,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    value: Option<Option<Value>>,
}

impl TryFrom<JsonData> for UpdateAsset {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let category_id = NullableUpdate::parse(value.category_id, AssetCategoryId::parse)?;
        let tracking_mode = value
            .tracking_mode
            .as_deref()
            .map(AssetTrackingMode::parse)
            .transpose()?;
        let name = value.name.map(AssetName::parse).transpose()?;

        Ok(Self {
            category_id,
            tracking_mode,
            name,
            model: parse_nullable_text(value.model),
            manufacturer: parse_nullable_text(value.manufacturer),
            inventory_unit_id: value.inventory_unit_id.map(Uuid::into),
            public_notes: parse_nullable_text(value.public_notes),
            internal_notes: parse_nullable_text(value.internal_notes),
        })
    }
}

impl From<ParameterValueJsonData> for AssetParameterValueInput {
    fn from(value: ParameterValueJsonData) -> Self {
        Self {
            parameter_type_id: value.parameter_type_id,
            value: value.value.flatten(),
        }
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
pub enum UpdateAssetError {
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

impl std::fmt::Debug for UpdateAssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UpdateAssetError {
    fn status_code(&self) -> StatusCode {
        match self {
            UpdateAssetError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UpdateAssetError::Forbidden(_) => StatusCode::FORBIDDEN,
            UpdateAssetError::NotFound(_) => StatusCode::NOT_FOUND,
            UpdateAssetError::ConflictError(_) => StatusCode::CONFLICT,
            UpdateAssetError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<AssetDatabaseError> for UpdateAssetError {
    fn from(error: AssetDatabaseError) -> Self {
        match error {
            AssetDatabaseError::Validation(message) => Self::ValidationError(message),
            AssetDatabaseError::Conflict(message) => Self::ConflictError(message),
            AssetDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Update an asset",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, asset_id=%asset_id)
)]
pub async fn update_asset(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    asset_id: AssetId,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateAssetError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::Asset,
        Action::Update(asset_id.into()),
    )
    .await?
    {
        return Err(UpdateAssetError::Forbidden(
            "You don't have permission to update this asset.".into(),
        ));
    }

    let mut payload = payload.into_inner();
    let parameter_values = payload.parameters.take().map(|parameters| {
        parameters
            .into_iter()
            .map(AssetParameterValueInput::from)
            .collect::<Vec<_>>()
    });
    let update_asset = UpdateAsset::try_from(payload).map_err(UpdateAssetError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_asset_for_update(&mut transaction, asset_id.into())
        .await?
        .ok_or(UpdateAssetError::NotFound("Asset not found".into()))?;
    let existing_parameters =
        fetch_parameter_values_for_asset_for_update(&mut transaction, existing.asset_id).await?;
    let laboratory_id = LaboratoryId::parse(existing.laboratory_id)
        .map_err(|e| UpdateAssetError::UnexpectedError(anyhow!("{e}")))?;

    let category_id = resolve_category_id(&update_asset, &existing)?;
    validate_category(&mut transaction, laboratory_id, category_id).await?;
    let tracking_mode = resolve_tracking_mode(&update_asset, &existing)?;
    let inventory_unit_id = update_asset
        .inventory_unit_id
        .map(Uuid::from)
        .unwrap_or(existing.inventory_unit_id);

    update_asset_in_database(
        &mut transaction,
        existing.asset_id,
        category_id,
        tracking_mode,
        update_asset
            .name
            .as_ref()
            .map(|name| name.as_ref())
            .unwrap_or(&existing.name),
        update_asset
            .model
            .resolve(existing.model.clone())
            .as_deref(),
        update_asset
            .manufacturer
            .resolve(existing.manufacturer.clone())
            .as_deref(),
        inventory_unit_id,
        update_asset
            .public_notes
            .resolve(existing.public_notes.clone())
            .as_deref(),
        update_asset
            .internal_notes
            .resolve(existing.internal_notes.clone())
            .as_deref(),
    )
    .await?;

    convert_inventory_quantities_to_unit(
        &mut transaction,
        existing.asset_id,
        tracking_mode,
        existing.inventory_unit_id,
        inventory_unit_id,
    )
    .await?;
    if let Some(parameter_values) = parameter_values.as_deref() {
        apply_asset_parameter_updates(
            &mut transaction,
            laboratory_id,
            existing.asset_id,
            parameter_values,
            true,
        )
        .await?;
    }
    validate_required_parameters(
        &mut transaction,
        laboratory_id,
        existing.asset_id,
        category_id,
    )
    .await?;

    let asset = fetch_asset_for_update(&mut transaction, existing.asset_id)
        .await?
        .ok_or(UpdateAssetError::UnexpectedError(anyhow!(
            "Updated asset not found"
        )))?;
    let inventory_items =
        fetch_inventory_items_for_asset_for_update(&mut transaction, asset.asset_id).await?;
    let parameters =
        fetch_parameter_values_for_asset_for_update(&mut transaction, asset.asset_id).await?;

    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Update,
        AuditResource::Asset,
        Some(asset.asset_id),
        update_asset_rollback_details(&existing, &existing_parameters),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update an asset.")?;

    Ok(HttpResponse::Ok().json(AssetResponse::from_parts(
        asset,
        Some(inventory_items),
        Some(parameters),
        true,
    )))
}

fn parse_nullable_text(value: Option<Option<String>>) -> NullableUpdate<String> {
    match value {
        Some(Some(value)) => {
            let value = value.trim().to_string();
            if value.is_empty() {
                NullableUpdate::Clear
            } else {
                NullableUpdate::Set(value)
            }
        }
        Some(None) => NullableUpdate::Clear,
        None => NullableUpdate::Unchanged,
    }
}

fn resolve_category_id(
    update_asset: &UpdateAsset,
    existing: &AssetRow,
) -> Result<Option<Uuid>, UpdateAssetError> {
    let current = existing
        .category_id
        .map(AssetCategoryId::parse)
        .transpose()
        .map_err(|e| UpdateAssetError::UnexpectedError(anyhow!("{e}")))?;

    Ok(update_asset
        .category_id
        .clone()
        .resolve(current)
        .map(Uuid::from))
}

fn resolve_tracking_mode(
    update_asset: &UpdateAsset,
    existing: &AssetRow,
) -> Result<AssetTrackingMode, UpdateAssetError> {
    let current = AssetTrackingMode::parse(&existing.tracking_mode)
        .map_err(UpdateAssetError::ValidationError)?;
    let tracking_mode = update_asset.tracking_mode.unwrap_or(current);
    if tracking_mode != current && existing.inventory_item_count > 0 {
        return Err(UpdateAssetError::ValidationError(
            "Cannot change tracking_mode while inventory items exist".into(),
        ));
    }

    Ok(tracking_mode)
}
