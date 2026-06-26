use super::model::{
    AssetModelError, AssetParameterValueInput, AssetResponse, apply_asset_parameter_updates,
    convert_inventory_items_to_unit, fetch_asset_for_update,
    fetch_inventory_items_for_asset_for_update, fetch_parameter_values_for_asset_for_update,
    map_database_error, update_asset_rollback_details, validate_category,
    validate_required_parameters,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetCategoryId, AssetId, AssetName, AssetTrackingMode, LaboratoryId, NullableUpdate,
    UpdateAsset, UserId,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
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
    default_unit_id: Option<Uuid>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    public_notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    internal_notes: Option<Option<String>>,
    is_archived: Option<bool>,
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
        Ok(Self::new(
            match value.category_id {
                Some(Some(category_id)) => {
                    NullableUpdate::Set(AssetCategoryId::parse(category_id)?)
                }
                Some(None) => NullableUpdate::Clear,
                None => NullableUpdate::Unchanged,
            },
            value
                .tracking_mode
                .as_deref()
                .map(AssetTrackingMode::parse)
                .transpose()?,
            value.name.map(AssetName::parse).transpose()?,
            parse_nullable_string(value.model),
            parse_nullable_string(value.manufacturer),
            value.default_unit_id,
            parse_nullable_string(value.public_notes),
            parse_nullable_string(value.internal_notes),
            value.is_archived,
        ))
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

#[tracing::instrument(
    name = "Update an asset",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, asset_id=%asset_id)
)]
pub async fn update_asset(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<AssetId>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateAssetError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(UpdateAssetError::UnexpectedError)?
        .ok_or(UpdateAssetError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
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
    let existing = fetch_asset_for_update(&mut transaction, Uuid::from(*asset_id))
        .await?
        .ok_or(UpdateAssetError::NotFound("Asset not found".into()))?;
    let existing_parameters =
        fetch_parameter_values_for_asset_for_update(&mut transaction, existing.asset_id).await?;
    let laboratory_id = LaboratoryId::parse(existing.laboratory_id)
        .map_err(|e| UpdateAssetError::UnexpectedError(anyhow!("{e}")))?;
    validate_update_permission(&actor, &laboratory_id)?;

    let category_id = update_asset
        .category_id
        .resolve(
            existing
                .category_id
                .map(|id| AssetCategoryId::parse(id).unwrap()),
        )
        .map(Uuid::from);
    validate_category(&mut transaction, laboratory_id, category_id)
        .await
        .map_err(map_model_error)?;

    let current_tracking_mode = AssetTrackingMode::parse(&existing.tracking_mode)
        .map_err(UpdateAssetError::ValidationError)?;
    let tracking_mode = update_asset.tracking_mode.unwrap_or(current_tracking_mode);
    if tracking_mode != current_tracking_mode && existing.inventory_item_count > 0 {
        return Err(UpdateAssetError::ValidationError(
            "Cannot change tracking_mode while inventory items exist".into(),
        ));
    }

    let default_unit_id = update_asset
        .default_unit_id
        .unwrap_or(existing.default_unit_id);

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
        default_unit_id,
        update_asset
            .public_notes
            .resolve(existing.public_notes.clone())
            .as_deref(),
        update_asset
            .internal_notes
            .resolve(existing.internal_notes.clone())
            .as_deref(),
        update_asset.is_archived.unwrap_or(existing.is_archived),
    )
    .await?;

    if default_unit_id != existing.default_unit_id {
        convert_inventory_items_to_unit(&mut transaction, existing.asset_id, default_unit_id)
            .await
            .map_err(map_model_error)?;
    }

    if let Some(parameter_values) = parameter_values.as_deref() {
        apply_asset_parameter_updates(
            &mut transaction,
            laboratory_id,
            existing.asset_id,
            parameter_values,
            true,
        )
        .await
        .map_err(map_model_error)?;
    }
    validate_required_parameters(
        &mut transaction,
        laboratory_id,
        existing.asset_id,
        category_id,
    )
    .await
    .map_err(map_model_error)?;

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
        &actor,
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
    )))
}

fn validate_update_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), UpdateAssetError> {
    if actor.can_write_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(UpdateAssetError::Forbidden(
            "You don't have permission to update this asset.".into(),
        ))
    }
}

#[allow(clippy::too_many_arguments)]
async fn update_asset_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
    category_id: Option<Uuid>,
    tracking_mode: AssetTrackingMode,
    name: &str,
    model: Option<&str>,
    manufacturer: Option<&str>,
    default_unit_id: Uuid,
    public_notes: Option<&str>,
    internal_notes: Option<&str>,
    is_archived: bool,
) -> Result<(), UpdateAssetError> {
    sqlx::query(
        r#"
        UPDATE assets
        SET
            category_id = $2,
            tracking_mode = $3,
            name = $4,
            model = $5,
            manufacturer = $6,
            default_unit_id = $7,
            public_notes = $8,
            internal_notes = $9,
            is_archived = $10,
            updated_at = now()
        WHERE asset_id = $1
        "#,
    )
    .bind(asset_id)
    .bind(category_id)
    .bind(tracking_mode.as_str())
    .bind(name)
    .bind(model)
    .bind(manufacturer)
    .bind(default_unit_id)
    .bind(public_notes)
    .bind(internal_notes)
    .bind(is_archived)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| map_model_error(map_database_error(e)))?;

    Ok(())
}

fn map_model_error(error: AssetModelError) -> UpdateAssetError {
    match error {
        AssetModelError::Validation(message) => UpdateAssetError::ValidationError(message),
        AssetModelError::Conflict(message) => UpdateAssetError::ConflictError(message),
        AssetModelError::Unexpected(error) => UpdateAssetError::UnexpectedError(error),
    }
}

fn parse_nullable_string(value: Option<Option<String>>) -> NullableUpdate<String> {
    match value {
        Some(Some(value)) => empty_to_nullable_update(value),
        Some(None) => NullableUpdate::Clear,
        None => NullableUpdate::Unchanged,
    }
}

fn empty_to_nullable_update(value: String) -> NullableUpdate<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        NullableUpdate::Clear
    } else {
        NullableUpdate::Set(value)
    }
}
