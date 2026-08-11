use super::model::{
    AssetForInventoryRow, InventoryItemResponse, InventoryItemRow,
    create_inventory_items_rollback_details,
};
use super::queries::{
    InventoryItemDatabaseError, fetch_asset_for_inventory_for_update, fetch_asset_laboratory_id,
    validate_location,
};
use super::service::{insert_inventory_item, next_serial_numbers};
use crate::access_control::AssetPathId;
use crate::access_control::{Action, Actor, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    AssetId, AssetTrackingMode, FileUploadId, InventoryItemSerialNumber, InventoryItemSerialSource,
    InventoryStatus, LaboratoryId, LocationId, NewAttachment, NewInventoryItem, NewInventoryItems,
    UserId, ensure_unique_uploads,
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

impl JsonData {
    /// Every upload the payload references, at the top level or on one of its
    /// serial items.
    fn upload_ids(&self) -> impl Iterator<Item = FileUploadId> {
        self.attachments
            .iter()
            .flatten()
            .chain(
                self.serial_items
                    .iter()
                    .flatten()
                    .flat_map(|item| item.attachments.iter().flatten()),
            )
            .map(AttachmentJsonData::upload_id)
    }

    /// Moves the attachments out of the payload, leaving behind only the fields
    /// that describe the inventory items themselves.
    fn take_attachments(
        &mut self,
    ) -> Result<InventoryItemAttachmentSource, CreateInventoryItemsError> {
        let per_serial_item = self
            .serial_items
            .iter_mut()
            .flatten()
            .map(|item| item.attachments.take())
            .collect();
        InventoryItemAttachmentSource::parse(self.attachments.take(), per_serial_item)
    }
}

/// The two spellings for attachments in a create payload: the top-level
/// `attachments` applies to the single item the request creates, while
/// `serial_items[i].attachments` supplies one group per serial number. Only one
/// spelling may be used, so both collapse here into "one attachment group per
/// created inventory item, in creation order".
enum InventoryItemAttachmentSource {
    /// The payload assigns no attachments.
    Empty,
    /// A single group, which is why it requires the request to create exactly one
    /// inventory item.
    Single([Vec<NewAttachment>; 1]),
    /// One group per `serial_items` entry, in the order `insert_serialized_items`
    /// creates the rows.
    PerSerialItem(Vec<Vec<NewAttachment>>),
}

impl InventoryItemAttachmentSource {
    fn parse(
        whole: Option<Vec<AttachmentJsonData>>,
        per_serial_item: Vec<Option<Vec<AttachmentJsonData>>>,
    ) -> Result<Self, CreateInventoryItemsError> {
        let whole = parse_attachments(whole)?;
        let per_serial_item = per_serial_item
            .into_iter()
            .map(parse_attachments)
            .collect::<Result<Vec<_>, _>>()?;
        let has_per_serial_item = per_serial_item.iter().any(|group| !group.is_empty());
        match (whole.is_empty(), has_per_serial_item) {
            (false, true) => Err(CreateInventoryItemsError::ValidationError(
                "attachments cannot be combined with serial_items attachments".into(),
            )),
            (false, false) => Ok(Self::Single([whole])),
            (true, true) => Ok(Self::PerSerialItem(per_serial_item)),
            (true, false) => Ok(Self::Empty),
        }
    }

    fn groups(&self) -> &[Vec<NewAttachment>] {
        match self {
            Self::Empty => &[],
            Self::Single(group) => group,
            Self::PerSerialItem(groups) => groups,
        }
    }

    /// Every attachment the request writes, for the request-wide uniqueness check.
    fn attachments(&self) -> impl Iterator<Item = &NewAttachment> {
        self.groups().iter().flatten()
    }

    /// Assigns each group to the inventory item created at the same position.
    async fn assign(
        self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: UserId,
        created: &[InventoryItemRow],
        laboratory_id: LaboratoryId,
    ) -> Result<(), CreateInventoryItemsError> {
        let (groups, mismatch) = match self {
            Self::Empty => return Ok(()),
            Self::Single(group) => (
                Vec::from(group),
                "attachments can only be supplied when exactly one inventory item is created",
            ),
            Self::PerSerialItem(groups) => (
                groups,
                "serial_items attachments must match created inventory items",
            ),
        };
        if groups.len() != created.len() {
            return Err(CreateInventoryItemsError::ValidationError(mismatch.into()));
        }

        for (item, group) in created.iter().zip(groups) {
            assign_uploaded_attachments(
                transaction,
                actor_user_id,
                AttachmentTarget::InventoryItem(item.inventory_item_id),
                Some(laboratory_id),
                &group,
            )
            .await?;
        }

        Ok(())
    }
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
    fields(actor_user_id=%laboratory_context.actor().user_id, asset_id=%asset_id)
)]
pub async fn create_inventory_items(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    asset_id: AssetPathId,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateInventoryItemsError> {
    let scope_laboratory_id = laboratory_context.laboratory_id();
    let actor = laboratory_context.authorization_actor();
    let asset_id: AssetId = asset_id.into_inner().into();
    let laboratory_id: LaboratoryId = fetch_asset_laboratory_id(&pool, asset_id.into())
        .await?
        .ok_or(CreateInventoryItemsError::NotFound(
            "Asset not found".into(),
        ))?
        .into();
    if laboratory_id != scope_laboratory_id {
        return Err(CreateInventoryItemsError::NotFound(
            "Asset not found".into(),
        ));
    }
    if !validate_permission(
        &pool,
        &actor,
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
    validate_upload_permissions(&pool, laboratory_context.actor(), &payload).await?;
    let attachments = payload.take_attachments()?;
    ensure_unique_uploads(attachments.attachments())
        .map_err(CreateInventoryItemsError::ValidationError)?;

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
    let creation = parse_creation(payload, tracking_mode)
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

    attachments
        .assign(
            &mut transaction,
            laboratory_context.actor().user_id,
            &created,
            asset.laboratory_id.into(),
        )
        .await?;

    record_audit(
        &mut transaction,
        laboratory_context.actor(),
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

/// Maps the payload onto the domain command field by field, so the long argument
/// list cannot be mixed up. The attachments must already have been taken out.
fn parse_creation(
    payload: JsonData,
    tracking_mode: AssetTrackingMode,
) -> Result<NewInventoryItems, String> {
    let JsonData {
        serial_items,
        serial_numbers,
        count,
        batch_number,
        quantity_on_hand,
        quantity_allocated,
        location_id,
        status,
        public_notes,
        internal_notes,
        attachments: _,
    } = payload;

    NewInventoryItems::parse(
        tracking_mode,
        serial_items.map(|items| items.into_iter().map(|item| item.serial_number).collect()),
        serial_numbers,
        count,
        batch_number,
        quantity_on_hand,
        quantity_allocated,
        location_id.map(Uuid::into),
        status,
        public_notes,
        internal_notes,
    )
}

/// Creating an inventory item does not by itself grant the right to claim an
/// upload, so every referenced upload is authorised individually.
async fn validate_upload_permissions(
    pool: &PgPool,
    actor: &Actor,
    payload: &JsonData,
) -> Result<(), CreateInventoryItemsError> {
    for upload_id in payload.upload_ids() {
        if !validate_permission(
            pool,
            actor,
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

#[allow(clippy::too_many_arguments)]
async fn insert_serialized_items(
    transaction: &mut Transaction<'_, Postgres>,
    asset: &AssetForInventoryRow,
    serial_source: InventoryItemSerialSource,
    batch_number: Option<String>,
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
        item.location_id.map(Uuid::from),
        item.status.as_str(),
        item.public_notes.as_deref(),
        item.internal_notes.as_deref(),
    )
    .await?)
}
