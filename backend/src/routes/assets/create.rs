use super::model::{
    AssetDatabaseError, AssetParameterValueInput, AssetResponse, AssetRow,
    apply_asset_parameter_updates, create_asset_rollback_details, fetch_asset_for_update,
    fetch_inventory_items_for_asset_for_update, fetch_parameter_values_for_asset_for_update,
    insert_inventory_items, map_database_error, validate_category, validate_required_parameters,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetName, AssetTrackingMode, LaboratoryId, NewAsset, NewAttachment, NewInventoryItem, UnitId,
    UserId,
};
use crate::routes::attachments::{
    AssignAttachmentError, AttachmentJsonData, AttachmentTarget, assign_uploaded_attachments,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use serde::Deserialize;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashSet;
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
    inventory_items: Option<Vec<InventoryItemJsonData>>,
    parameters: Option<Vec<ParameterValueJsonData>>,
    attachments: Option<Vec<AttachmentJsonData>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterValueJsonData {
    parameter_type_id: Uuid,
    value: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryItemJsonData {
    serial_number: Option<String>,
    batch_number: Option<String>,
    quantity_on_hand: Option<f64>,
    quantity_allocated: Option<f64>,
    quantity_unit_id: Option<Uuid>,
    location_id: Option<Uuid>,
    status: Option<String>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
    attachments: Option<Vec<AttachmentJsonData>>,
}

impl TryFrom<JsonData> for NewAsset {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        Ok(Self::new(
            value.category_id.map(Uuid::into),
            AssetTrackingMode::parse(&value.tracking_mode)?,
            AssetName::parse(value.name)?,
            empty_to_none(value.model),
            empty_to_none(value.manufacturer),
            value.default_unit_id.into(),
            empty_to_none(value.public_notes),
            empty_to_none(value.internal_notes),
        ))
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

impl From<AssetDatabaseError> for CreateAssetError {
    fn from(error: AssetDatabaseError) -> Self {
        match error {
            AssetDatabaseError::Validation(message) => Self::ValidationError(message),
            AssetDatabaseError::Conflict(message) => Self::ConflictError(message),
            AssetDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

impl From<AssignAttachmentError> for CreateAssetError {
    fn from(error: AssignAttachmentError) -> Self {
        match error {
            AssignAttachmentError::ValidationError(message) => Self::ValidationError(message),
            AssignAttachmentError::Forbidden(message) => Self::Forbidden(message),
            // An upload referenced by a create payload that does not exist is bad
            // input, not a missing asset, so it stays a 400 here.
            AssignAttachmentError::NotFound(message) => Self::ValidationError(message),
            AssignAttachmentError::ConflictError(message) => Self::ConflictError(message),
            AssignAttachmentError::UnexpectedError(error) => Self::UnexpectedError(error),
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
    let laboratory_id: LaboratoryId = laboratory_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::Asset,
        Action::Create(laboratory_id.into()),
    )
    .await?
    {
        return Err(CreateAssetError::Forbidden(
            "You don't have permission to create assets.".into(),
        ));
    }

    let mut payload = payload.into_inner();
    validate_upload_permissions(&pool, &actor_user_id, &payload).await?;
    let asset_attachments = parse_attachments(payload.attachments.take())?;
    let parameter_values: Vec<_> = payload
        .parameters
        .take()
        .unwrap_or_default()
        .into_iter()
        .map(AssetParameterValueInput::from)
        .collect();
    let inventory_payloads = payload.inventory_items.take().unwrap_or_default();
    let new_asset = NewAsset::try_from(payload).map_err(CreateAssetError::ValidationError)?;

    let mut inventory_items = Vec::with_capacity(inventory_payloads.len());
    let mut inventory_attachments = Vec::with_capacity(inventory_payloads.len());
    for mut inventory_payload in inventory_payloads {
        inventory_attachments.push(parse_attachments(inventory_payload.attachments.take())?);
        inventory_items.push(
            parse_inventory_item(
                inventory_payload,
                new_asset.tracking_mode,
                new_asset.default_unit_id,
            )
            .map_err(CreateAssetError::ValidationError)?,
        );
    }
    validate_unique_uploads(&asset_attachments, &inventory_attachments)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    validate_category(
        &mut transaction,
        laboratory_id,
        new_asset.category_id.map(Uuid::from),
    )
    .await?;
    let asset = insert_asset(&mut transaction, laboratory_id, &new_asset).await?;
    let created_inventory_items = insert_inventory_items(
        &mut transaction,
        laboratory_id,
        asset.asset_id,
        new_asset.tracking_mode,
        &inventory_items,
    )
    .await?;
    assign_uploaded_attachments(
        &mut transaction,
        actor_user_id,
        AttachmentTarget::Asset(asset.asset_id),
        Some(laboratory_id),
        &asset_attachments,
    )
    .await?;
    for (item, attachments) in created_inventory_items
        .iter()
        .zip(inventory_attachments.iter())
    {
        assign_uploaded_attachments(
            &mut transaction,
            actor_user_id,
            AttachmentTarget::InventoryItem(item.inventory_item_id),
            Some(laboratory_id),
            attachments,
        )
        .await?;
    }
    apply_asset_parameter_updates(
        &mut transaction,
        laboratory_id,
        asset.asset_id,
        &parameter_values,
        false,
    )
    .await?;
    validate_required_parameters(
        &mut transaction,
        laboratory_id,
        asset.asset_id,
        asset.category_id,
    )
    .await?;

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
        actor_user_id,
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
        true,
    )))
}

fn parse_attachments(
    attachments: Option<Vec<AttachmentJsonData>>,
) -> Result<Vec<NewAttachment>, CreateAssetError> {
    attachments
        .unwrap_or_default()
        .into_iter()
        .map(NewAttachment::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(CreateAssetError::ValidationError)
}

/// Creating an asset does not by itself grant the right to claim an upload, so
/// every referenced upload is authorised individually.
async fn validate_upload_permissions(
    pool: &PgPool,
    actor_user_id: &UserId,
    payload: &JsonData,
) -> Result<(), CreateAssetError> {
    let upload_ids = payload
        .attachments
        .iter()
        .flatten()
        .chain(
            payload
                .inventory_items
                .iter()
                .flatten()
                .flat_map(|item| item.attachments.iter().flatten()),
        )
        .map(AttachmentJsonData::upload_id);
    for upload_id in upload_ids {
        if !validate_permission(
            pool,
            actor_user_id,
            ResourceType::FileUpload,
            Action::Assign(upload_id.into()),
        )
        .await?
        {
            return Err(CreateAssetError::Forbidden(
                "You do not have permission to assign this attachment".into(),
            ));
        }
    }

    Ok(())
}

/// An upload can only become one attachment, so it may appear at most once across
/// the asset and all of its inventory items.
fn validate_unique_uploads(
    asset_attachments: &[NewAttachment],
    inventory_attachments: &[Vec<NewAttachment>],
) -> Result<(), CreateAssetError> {
    let mut upload_ids = HashSet::new();
    for attachment in asset_attachments
        .iter()
        .chain(inventory_attachments.iter().flatten())
    {
        if !upload_ids.insert(attachment.upload_id) {
            return Err(CreateAssetError::ValidationError(
                "An upload can only be assigned once in a create request".into(),
            ));
        }
    }

    Ok(())
}

fn parse_inventory_item(
    value: InventoryItemJsonData,
    tracking_mode: AssetTrackingMode,
    default_unit_id: UnitId,
) -> Result<NewInventoryItem, String> {
    NewInventoryItem::parse_for_tracking_mode(
        tracking_mode,
        value.serial_number,
        value.batch_number,
        value.quantity_on_hand,
        value.quantity_allocated,
        value.quantity_unit_id.map(Uuid::into),
        default_unit_id,
        value.location_id.map(Uuid::into),
        value.status,
        value.public_notes,
        value.internal_notes,
    )
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
) -> Result<AssetRow, CreateAssetError> {
    sqlx::query_as!(
        AssetRow,
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
            internal_notes
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
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
            created_at,
            updated_at,
            0::bigint AS "inventory_item_count!",
            0::double precision AS "quantity_on_hand!",
            0::double precision AS "quantity_allocated!"
        "#,
        Uuid::new_v4(),
        *laboratory_id,
        new_asset.category_id.map(Uuid::from),
        new_asset.tracking_mode.as_str(),
        new_asset.name.as_ref(),
        new_asset.model.as_deref(),
        new_asset.manufacturer.as_deref(),
        Uuid::from(new_asset.default_unit_id),
        new_asset.public_notes.as_deref(),
        new_asset.internal_notes.as_deref(),
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(|e| map_database_error(e).into())
}
