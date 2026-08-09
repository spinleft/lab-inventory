use super::model::{
    AssetForInventoryRow, InventoryItemDatabaseError, InventoryItemResponse, InventoryItemRow,
    create_inventory_items_rollback_details, fetch_asset_for_inventory_for_update,
    fetch_asset_laboratory_id, insert_inventory_item, next_serial_numbers, validate_location,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetId, AssetTrackingMode, InventoryItemSerialNumber, InventoryItemSerialSource,
    InventoryStatus, LaboratoryId, LocationId, NewAttachment, NewInventoryItem, NewInventoryItems,
    UnitId, UserId,
};
use crate::routes::attachments::{
    AssignAttachmentError, AttachmentJsonData, AttachmentTarget, assign_uploaded_attachments,
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
    serial_items: Option<Vec<SerialItemJsonData>>,
    serial_numbers: Option<Vec<String>>,
    count: Option<i64>,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SerialItemJsonData {
    serial_number: String,
    attachments: Option<Vec<AttachmentJsonData>>,
}

#[derive(thiserror::Error)]
pub enum CreateInventoryItemsError {
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

impl std::fmt::Debug for CreateInventoryItemsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateInventoryItemsError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateInventoryItemsError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateInventoryItemsError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateInventoryItemsError::NotFound(_) => StatusCode::NOT_FOUND,
            CreateInventoryItemsError::ConflictError(_) => StatusCode::CONFLICT,
            CreateInventoryItemsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<InventoryItemDatabaseError> for CreateInventoryItemsError {
    fn from(error: InventoryItemDatabaseError) -> Self {
        match error {
            InventoryItemDatabaseError::Validation(message) => Self::ValidationError(message),
            InventoryItemDatabaseError::NotFound(message) => Self::NotFound(message),
            InventoryItemDatabaseError::Conflict(message) => Self::ConflictError(message),
            InventoryItemDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

impl From<AssignAttachmentError> for CreateInventoryItemsError {
    fn from(error: AssignAttachmentError) -> Self {
        match error {
            AssignAttachmentError::ValidationError(message) => Self::ValidationError(message),
            AssignAttachmentError::Forbidden(message) => Self::Forbidden(message),
            // An upload referenced by a create payload that does not exist is bad
            // input, not a missing inventory item, so it stays a 400 here.
            AssignAttachmentError::NotFound(message) => Self::ValidationError(message),
            AssignAttachmentError::ConflictError(message) => Self::ConflictError(message),
            AssignAttachmentError::UnexpectedError(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Create inventory items",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, asset_id=%asset_id)
)]
pub async fn create_inventory_items(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateInventoryItemsError> {
    let asset_id: AssetId = asset_id.into_inner().into();
    let laboratory_id: LaboratoryId = fetch_asset_laboratory_id(&pool, asset_id.into())
        .await?
        .ok_or(CreateInventoryItemsError::NotFound(
            "Asset not found".into(),
        ))?
        .into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::InventoryItem,
        Action::Create(laboratory_id.into()),
    )
    .await?
    {
        return Err(CreateInventoryItemsError::Forbidden(
            "You don't have permission to create inventory items.".into(),
        ));
    }

    let mut payload = payload.into_inner();
    validate_upload_permissions(&pool, &actor_user_id, &payload).await?;
    let attachments = parse_attachments(payload.attachments.take())?;
    let serial_item_attachments = payload
        .serial_items
        .as_ref()
        .map(|items| {
            items
                .iter()
                .map(|item| parse_attachments(item.attachments.clone()))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    validate_unique_uploads(&attachments, &serial_item_attachments)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let asset = fetch_asset_for_inventory_for_update(&mut transaction, asset_id.into())
        .await?
        .ok_or(CreateInventoryItemsError::NotFound(
            "Asset not found".into(),
        ))?;
    let tracking_mode = AssetTrackingMode::parse(&asset.tracking_mode)
        .map_err(CreateInventoryItemsError::ValidationError)?;
    let creation = NewInventoryItems::parse(
        tracking_mode,
        payload
            .serial_items
            .map(|items| items.into_iter().map(|item| item.serial_number).collect()),
        payload.serial_numbers,
        payload.count,
        payload.batch_number,
        payload.quantity_on_hand,
        payload.quantity_allocated,
        payload.quantity_unit_id.map(Uuid::into),
        UnitId(asset.default_unit_id),
        payload.location_id.map(Uuid::into),
        payload.status,
        payload.public_notes,
        payload.internal_notes,
    )
    .map_err(CreateInventoryItemsError::ValidationError)?;
    if let Some(location_id) = creation.location_id() {
        validate_location(
            &mut transaction,
            asset.laboratory_id,
            Uuid::from(location_id),
        )
        .await?;
    }

    let created = match creation {
        NewInventoryItems::Serialized {
            serial_source,
            batch_number,
            quantity_unit_id,
            location_id,
            status,
            public_notes,
            internal_notes,
        } => {
            insert_serialized_items(
                &mut transaction,
                &asset,
                serial_source,
                batch_number,
                quantity_unit_id,
                location_id,
                status,
                public_notes,
                internal_notes,
            )
            .await?
        }
        NewInventoryItems::Quantity(item) => {
            vec![insert_new_inventory_item(&mut transaction, &asset, "quantity", &item).await?]
        }
    };

    if !attachments.is_empty() {
        validate_attachment_targets(&created, &serial_item_attachments)?;
        assign_uploaded_attachments(
            &mut transaction,
            actor_user_id,
            AttachmentTarget::InventoryItem(created[0].inventory_item_id),
            Some(asset.laboratory_id.into()),
            &attachments,
        )
        .await?;
    }
    if serial_item_attachments
        .iter()
        .any(|attachments| !attachments.is_empty())
    {
        if created.len() != serial_item_attachments.len() {
            return Err(CreateInventoryItemsError::ValidationError(
                "serial_items attachments must match created inventory items".into(),
            ));
        }
        for (item, attachments) in created.iter().zip(serial_item_attachments.iter()) {
            assign_uploaded_attachments(
                &mut transaction,
                actor_user_id,
                AttachmentTarget::InventoryItem(item.inventory_item_id),
                Some(asset.laboratory_id.into()),
                attachments,
            )
            .await?;
        }
    }

    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Create,
        AuditResource::InventoryItem,
        Some(created[0].inventory_item_id),
        create_inventory_items_rollback_details(&created),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to create inventory items.")?;

    Ok(HttpResponse::Created().json(
        created
            .into_iter()
            .map(|item| InventoryItemResponse::from_row(item, true))
            .collect::<Vec<_>>(),
    ))
}

fn parse_attachments(
    attachments: Option<Vec<AttachmentJsonData>>,
) -> Result<Vec<NewAttachment>, CreateInventoryItemsError> {
    attachments
        .unwrap_or_default()
        .into_iter()
        .map(NewAttachment::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(CreateInventoryItemsError::ValidationError)
}

/// Creating an inventory item does not by itself grant the right to claim an
/// upload, so every referenced upload is authorised individually.
async fn validate_upload_permissions(
    pool: &PgPool,
    actor_user_id: &UserId,
    payload: &JsonData,
) -> Result<(), CreateInventoryItemsError> {
    let upload_ids = payload
        .attachments
        .iter()
        .flatten()
        .chain(
            payload
                .serial_items
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
            return Err(CreateInventoryItemsError::Forbidden(
                "You do not have permission to assign this attachment".into(),
            ));
        }
    }

    Ok(())
}

fn validate_unique_uploads(
    attachments: &[NewAttachment],
    serial_item_attachments: &[Vec<NewAttachment>],
) -> Result<(), CreateInventoryItemsError> {
    let mut upload_ids = HashSet::new();
    for attachment in attachments
        .iter()
        .chain(serial_item_attachments.iter().flatten())
    {
        if !upload_ids.insert(attachment.upload_id) {
            return Err(CreateInventoryItemsError::ValidationError(
                "An upload can only be assigned once in a create request".into(),
            ));
        }
    }

    Ok(())
}

fn validate_attachment_targets(
    created: &[InventoryItemRow],
    serial_item_attachments: &[Vec<NewAttachment>],
) -> Result<(), CreateInventoryItemsError> {
    if serial_item_attachments
        .iter()
        .any(|attachments| !attachments.is_empty())
    {
        return Err(CreateInventoryItemsError::ValidationError(
            "attachments cannot be combined with serial_items attachments".into(),
        ));
    }
    if created.len() != 1 {
        return Err(CreateInventoryItemsError::ValidationError(
            "attachments can only be supplied when exactly one inventory item is created".into(),
        ));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_serialized_items(
    transaction: &mut Transaction<'_, Postgres>,
    asset: &AssetForInventoryRow,
    serial_source: InventoryItemSerialSource,
    batch_number: Option<String>,
    quantity_unit_id: UnitId,
    location_id: Option<LocationId>,
    status: InventoryStatus,
    public_notes: Option<String>,
    internal_notes: Option<String>,
) -> Result<Vec<InventoryItemRow>, CreateInventoryItemsError> {
    let serial_numbers = match serial_source {
        InventoryItemSerialSource::Explicit(values) => values,
        InventoryItemSerialSource::Generate(count) => {
            next_serial_numbers(transaction, asset.asset_id, count)
                .await?
                .into_iter()
                .map(InventoryItemSerialNumber::parse)
                .collect::<Result<Vec<_>, _>>()
                .map_err(CreateInventoryItemsError::ValidationError)?
        }
    };

    let mut created = Vec::with_capacity(serial_numbers.len());
    for serial_number in serial_numbers {
        let item = NewInventoryItem::serialized(
            serial_number,
            batch_number.clone(),
            quantity_unit_id,
            location_id,
            status,
            public_notes.clone(),
            internal_notes.clone(),
        );
        created.push(insert_new_inventory_item(transaction, asset, "serialized", &item).await?);
    }

    Ok(created)
}

async fn insert_new_inventory_item(
    transaction: &mut Transaction<'_, Postgres>,
    asset: &AssetForInventoryRow,
    tracking_mode: &str,
    item: &NewInventoryItem,
) -> Result<InventoryItemRow, CreateInventoryItemsError> {
    Ok(insert_inventory_item(
        transaction,
        asset.asset_id,
        asset.laboratory_id,
        tracking_mode,
        item.serial_number.as_ref().map(AsRef::as_ref),
        item.batch_number.as_deref(),
        item.quantity_on_hand,
        item.quantity_allocated,
        item.quantity_unit_id.into(),
        item.location_id.map(Uuid::from),
        item.status.as_str(),
        item.public_notes.as_deref(),
        item.internal_notes.as_deref(),
    )
    .await?)
}
