use super::model::{AssetParameterValueInput, AssetResponse, create_asset_rollback_details};
use super::queries::{
    AssetDatabaseError, fetch_asset_for_update, fetch_inventory_items_for_asset_for_update,
    fetch_parameter_values_for_asset_for_update, insert_asset, validate_category,
};
use super::service::{
    apply_asset_parameter_updates, insert_asset_inventory_item, validate_required_parameters,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetName, AssetTrackingMode, FileUploadId, LaboratoryId, NewAsset, NewAttachment,
    NewInventoryItem, UserId, ensure_unique_uploads,
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
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    category_id: Option<Uuid>,
    tracking_mode: String,
    name: String,
    model: Option<String>,
    manufacturer: Option<String>,
    inventory_unit_id: Uuid,
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
    location_id: Option<Uuid>,
    status: Option<String>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
    attachments: Option<Vec<AttachmentJsonData>>,
}

impl JsonData {
    /// Every upload the payload references, on the asset itself or on one of its
    /// inventory items.
    fn upload_ids(&self) -> impl Iterator<Item = FileUploadId> {
        self.attachments
            .iter()
            .flatten()
            .chain(
                self.inventory_items
                    .iter()
                    .flatten()
                    .flat_map(|item| item.attachments.iter().flatten()),
            )
            .map(AttachmentJsonData::upload_id)
    }
}

/// The whole create request after validation: the asset and everything that is
/// written alongside it in the same transaction.
struct CreateAssetInput {
    asset: NewAsset,
    attachments: Vec<NewAttachment>,
    parameter_values: Vec<AssetParameterValueInput>,
    inventory_items: Vec<NewInventoryItemInput>,
}

/// One inventory item together with the attachments assigned to it.
struct NewInventoryItemInput {
    item: NewInventoryItem,
    attachments: Vec<NewAttachment>,
}

impl TryFrom<JsonData> for CreateAssetInput {
    type Error = String;

    fn try_from(mut payload: JsonData) -> Result<Self, Self::Error> {
        let attachments = parse_attachments(payload.attachments.take())?;
        let parameter_values = payload
            .parameters
            .take()
            .unwrap_or_default()
            .into_iter()
            .map(AssetParameterValueInput::from)
            .collect();
        let item_payloads = payload.inventory_items.take().unwrap_or_default();
        let asset = NewAsset::try_from(payload)?;

        let inventory_items = item_payloads
            .into_iter()
            .map(|item| NewInventoryItemInput::parse(item, asset.tracking_mode))
            .collect::<Result<Vec<_>, _>>()?;

        let input = Self {
            asset,
            attachments,
            parameter_values,
            inventory_items,
        };
        ensure_unique_uploads(input.all_attachments())?;

        Ok(input)
    }
}

impl CreateAssetInput {
    /// Every attachment the request writes, on the asset itself or on one of its
    /// inventory items.
    fn all_attachments(&self) -> impl Iterator<Item = &NewAttachment> {
        self.attachments.iter().chain(
            self.inventory_items
                .iter()
                .flat_map(|item| item.attachments.iter()),
        )
    }
}

impl NewInventoryItemInput {
    fn parse(
        mut payload: InventoryItemJsonData,
        tracking_mode: AssetTrackingMode,
    ) -> Result<Self, String> {
        let attachments = parse_attachments(payload.attachments.take())?;
        let item = NewInventoryItem::parse_for_tracking_mode(
            tracking_mode,
            payload.serial_number,
            payload.batch_number,
            payload.quantity_on_hand,
            payload.quantity_allocated,
            payload.location_id.map(Uuid::into),
            payload.status,
            payload.public_notes,
            payload.internal_notes,
        )?;

        Ok(Self { item, attachments })
    }
}

impl TryFrom<JsonData> for NewAsset {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        Ok(Self {
            category_id: value.category_id.map(Uuid::into),
            tracking_mode: AssetTrackingMode::parse(&value.tracking_mode)?,
            name: AssetName::parse(value.name)?,
            model: empty_to_none(value.model),
            manufacturer: empty_to_none(value.manufacturer),
            inventory_unit_id: value.inventory_unit_id.into(),
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

fn parse_attachments(
    attachments: Option<Vec<AttachmentJsonData>>,
) -> Result<Vec<NewAttachment>, String> {
    attachments
        .unwrap_or_default()
        .into_iter()
        .map(NewAttachment::try_from)
        .collect()
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

    let payload = payload.into_inner();
    validate_upload_permissions(&pool, &actor_user_id, payload.upload_ids()).await?;
    let input = CreateAssetInput::try_from(payload).map_err(CreateAssetError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let asset_id =
        insert_asset_graph(&mut transaction, actor_user_id, laboratory_id, &input).await?;

    let asset = fetch_asset_for_update(&mut transaction, asset_id)
        .await?
        .ok_or(CreateAssetError::UnexpectedError(anyhow!(
            "Created asset not found"
        )))?;
    let inventory_items =
        fetch_inventory_items_for_asset_for_update(&mut transaction, asset_id).await?;
    let parameters =
        fetch_parameter_values_for_asset_for_update(&mut transaction, asset_id).await?;

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

/// Writes the asset and everything hanging off it, returning the new asset id.
async fn insert_asset_graph(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: UserId,
    laboratory_id: LaboratoryId,
    input: &CreateAssetInput,
) -> Result<Uuid, CreateAssetError> {
    validate_category(
        transaction,
        laboratory_id,
        input.asset.category_id.map(Uuid::from),
    )
    .await?;
    let asset = insert_asset(transaction, laboratory_id, &input.asset).await?;
    assign_uploaded_attachments(
        transaction,
        actor_user_id,
        AttachmentTarget::Asset(asset.asset_id),
        Some(laboratory_id),
        &input.attachments,
    )
    .await?;

    for inventory_item in &input.inventory_items {
        let row = insert_asset_inventory_item(
            transaction,
            laboratory_id,
            asset.asset_id,
            input.asset.tracking_mode,
            &inventory_item.item,
        )
        .await?;
        assign_uploaded_attachments(
            transaction,
            actor_user_id,
            AttachmentTarget::InventoryItem(row.inventory_item_id),
            Some(laboratory_id),
            &inventory_item.attachments,
        )
        .await?;
    }

    apply_asset_parameter_updates(
        transaction,
        laboratory_id,
        asset.asset_id,
        &input.parameter_values,
        false,
    )
    .await?;
    validate_required_parameters(
        transaction,
        laboratory_id,
        asset.asset_id,
        asset.category_id,
    )
    .await?;

    Ok(asset.asset_id)
}

/// Creating an asset does not by itself grant the right to assign an upload, so
/// every referenced upload is authorised individually.
async fn validate_upload_permissions(
    pool: &PgPool,
    actor_user_id: &UserId,
    upload_ids: impl Iterator<Item = FileUploadId>,
) -> Result<(), CreateAssetError> {
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
