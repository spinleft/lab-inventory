use super::model::{
    AssetInventoryItemInput, AssetModelError, AssetParameterValueInput, AssetResponse,
    apply_asset_parameter_updates, create_asset_rollback_details, fetch_asset_for_update,
    fetch_inventory_items_for_asset_for_update, fetch_parameter_values_for_asset_for_update,
    insert_inventory_items, map_database_error, validate_category, validate_required_parameters,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetCategoryId, AssetInventoryStatus, AssetName, AssetTrackingMode, AttachmentClaim,
    LaboratoryId, NewAsset, UserId,
};
use crate::routes::attachments::{
    AttachmentClaimInput, AttachmentError, claim_asset_attachments,
    claim_inventory_item_attachments,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use serde::Deserialize;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    category_id: Option<Uuid>,
    tracking_mode: String,
    name: String,
    model: Option<String>,
    manufacturer: Option<String>,
    default_unit_id: Uuid,
    public_notes: Option<String>,
    internal_notes: Option<String>,
    is_archived: Option<bool>,
    inventory_items: Option<Vec<InventoryItemJsonData>>,
    parameters: Option<Vec<ParameterValueJsonData>>,
    attachments: Option<Vec<AttachmentClaimInput>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryItemJsonData {
    serial_number: Option<String>,
    batch_number: Option<String>,
    quantity_on_hand: Option<f64>,
    quantity_allocated: Option<f64>,
    quantity_unit_id: Option<Uuid>,
    location_id: Option<Uuid>,
    status: Option<String>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
    attachments: Option<Vec<AttachmentClaimInput>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterValueJsonData {
    parameter_type_id: Uuid,
    value: Value,
}

impl TryFrom<JsonData> for NewAsset {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        Ok(Self::new(
            value.category_id.map(AssetCategoryId::parse).transpose()?,
            AssetTrackingMode::parse(&value.tracking_mode)?,
            AssetName::parse(value.name)?,
            empty_to_none(value.model),
            empty_to_none(value.manufacturer),
            value.default_unit_id,
            empty_to_none(value.public_notes),
            empty_to_none(value.internal_notes),
            value.is_archived.unwrap_or(false),
        ))
    }
}

impl TryFrom<InventoryItemJsonData> for AssetInventoryItemInput {
    type Error = String;

    fn try_from(value: InventoryItemJsonData) -> Result<Self, Self::Error> {
        let status = match value.status {
            Some(status) => AssetInventoryStatus::parse(&status)?.as_str().to_string(),
            None => "available".to_string(),
        };
        Ok(Self {
            serial_number: empty_to_none(value.serial_number),
            batch_number: empty_to_none(value.batch_number),
            quantity_on_hand: value.quantity_on_hand,
            quantity_allocated: value.quantity_allocated,
            quantity_unit_id: value.quantity_unit_id,
            location_id: value.location_id,
            status,
            public_notes: empty_to_none(value.public_notes),
            internal_notes: empty_to_none(value.internal_notes),
        })
    }
}

impl From<ParameterValueJsonData> for AssetParameterValueInput {
    fn from(value: ParameterValueJsonData) -> Self {
        Self {
            parameter_type_id: value.parameter_type_id,
            value: Some(value.value),
        }
    }
}

#[derive(thiserror::Error)]
pub enum CreateAssetError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreateAssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateAssetError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateAssetError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateAssetError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateAssetError::ConflictError(_) => StatusCode::CONFLICT,
            CreateAssetError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Create an asset",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn create_asset(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateAssetError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id.into_inner())
        .map_err(CreateAssetError::ValidationError)?;
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(CreateAssetError::UnexpectedError)?
        .ok_or(CreateAssetError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_create_permission(&actor, &laboratory_id)?;

    let mut payload = payload.into_inner();
    let asset_attachment_claims =
        parse_attachment_claims(payload.attachments.take().unwrap_or_default())?;
    let inventory_payloads = payload.inventory_items.take().unwrap_or_default();
    let mut inventory_items = Vec::with_capacity(inventory_payloads.len());
    let mut inventory_attachment_claims = Vec::with_capacity(inventory_payloads.len());
    for mut inventory_payload in inventory_payloads {
        let attachments =
            parse_attachment_claims(inventory_payload.attachments.take().unwrap_or_default())?;
        inventory_items.push(
            AssetInventoryItemInput::try_from(inventory_payload)
                .map_err(CreateAssetError::ValidationError)?,
        );
        inventory_attachment_claims.push(attachments);
    }
    let parameter_values = payload
        .parameters
        .take()
        .unwrap_or_default()
        .into_iter()
        .map(AssetParameterValueInput::from)
        .collect::<Vec<_>>();
    let new_asset = NewAsset::try_from(payload).map_err(CreateAssetError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    validate_category(
        &mut transaction,
        laboratory_id,
        new_asset.category_id.map(Uuid::from),
    )
    .await
    .map_err(map_model_error)?;
    let asset = insert_asset(&mut transaction, laboratory_id, &new_asset).await?;
    let created_inventory_items = insert_inventory_items(
        &mut transaction,
        laboratory_id,
        asset.asset_id,
        new_asset.tracking_mode,
        asset.default_unit_id,
        &inventory_items,
    )
    .await
    .map_err(map_model_error)?;
    claim_asset_attachments(
        &mut transaction,
        &actor,
        laboratory_id,
        asset.asset_id,
        &asset_attachment_claims,
    )
    .await
    .map_err(map_attachment_error)?;
    for (item, claims) in created_inventory_items
        .iter()
        .zip(inventory_attachment_claims.iter())
    {
        claim_inventory_item_attachments(
            &mut transaction,
            &actor,
            laboratory_id,
            item.inventory_item_id,
            claims,
        )
        .await
        .map_err(map_attachment_error)?;
    }
    apply_asset_parameter_updates(
        &mut transaction,
        laboratory_id,
        asset.asset_id,
        &parameter_values,
        false,
    )
    .await
    .map_err(map_model_error)?;
    validate_required_parameters(
        &mut transaction,
        laboratory_id,
        asset.asset_id,
        asset.category_id,
    )
    .await
    .map_err(map_model_error)?;

    let asset = fetch_asset_for_update(&mut transaction, asset.asset_id)
        .await?
        .ok_or(CreateAssetError::UnexpectedError(anyhow!(
            "Created asset not found"
        )))?;
    let inventory_items =
        fetch_inventory_items_for_asset_for_update(&mut transaction, asset.asset_id).await?;
    let parameters =
        fetch_parameter_values_for_asset_for_update(&mut transaction, asset.asset_id).await?;

    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Create,
        AuditResource::Asset,
        Some(asset.asset_id),
        create_asset_rollback_details(&asset),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new asset.")?;

    Ok(HttpResponse::Created().json(AssetResponse::from_parts(
        asset,
        Some(inventory_items),
        Some(parameters),
    )))
}

fn validate_create_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), CreateAssetError> {
    if actor.can_write_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(CreateAssetError::Forbidden(
            "You don't have permission to create assets for this laboratory.".into(),
        ))
    }
}

#[tracing::instrument(
    name = "Saving new asset in the database",
    skip(transaction, new_asset),
    fields(laboratory_id=%laboratory_id)
)]
async fn insert_asset(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    new_asset: &NewAsset,
) -> Result<super::model::AssetRow, CreateAssetError> {
    sqlx::query_as::<_, super::model::AssetRow>(
        r#"
        INSERT INTO assets (
            asset_id,
            laboratory_id,
            category_id,
            tracking_mode,
            name,
            model,
            manufacturer,
            default_unit_id,
            public_notes,
            internal_notes,
            is_archived
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING
            asset_id,
            laboratory_id,
            category_id,
            tracking_mode,
            name,
            model,
            manufacturer,
            default_unit_id,
            public_notes,
            internal_notes,
            is_archived,
            created_at,
            updated_at,
            0::bigint AS inventory_item_count,
            0::double precision AS quantity_on_hand,
            0::double precision AS quantity_allocated
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(*laboratory_id)
    .bind(new_asset.category_id.map(Uuid::from))
    .bind(new_asset.tracking_mode.as_str())
    .bind(new_asset.name.as_ref())
    .bind(new_asset.model.as_deref())
    .bind(new_asset.manufacturer.as_deref())
    .bind(new_asset.default_unit_id)
    .bind(new_asset.public_notes.as_deref())
    .bind(new_asset.internal_notes.as_deref())
    .bind(new_asset.is_archived)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(|e| map_model_error(map_database_error(e)))
}

fn map_model_error(error: AssetModelError) -> CreateAssetError {
    match error {
        AssetModelError::Validation(message) => CreateAssetError::ValidationError(message),
        AssetModelError::Conflict(message) => CreateAssetError::ConflictError(message),
        AssetModelError::Unexpected(error) => CreateAssetError::UnexpectedError(error),
    }
}

fn map_attachment_error(error: AttachmentError) -> CreateAssetError {
    match error {
        AttachmentError::ValidationError(message) => CreateAssetError::ValidationError(message),
        AttachmentError::Forbidden(message) => CreateAssetError::Forbidden(message),
        AttachmentError::NotFound(message) => CreateAssetError::ValidationError(message),
        AttachmentError::ConflictError(message) => CreateAssetError::ConflictError(message),
        AttachmentError::UnexpectedError(error) => CreateAssetError::UnexpectedError(error),
    }
}

fn parse_attachment_claims(
    claims: Vec<AttachmentClaimInput>,
) -> Result<Vec<AttachmentClaim>, CreateAssetError> {
    claims
        .into_iter()
        .map(AttachmentClaim::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(CreateAssetError::ValidationError)
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
