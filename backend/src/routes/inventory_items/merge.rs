use super::model::{
    InventoryItemResponse, InventoryItemRow, merge_inventory_items_rollback_details,
};
use super::queries::{
    InventoryItemDatabaseError, delete_inventory_item_from_database,
    fetch_inventory_items_for_update, move_inventory_item_attachments,
};
use super::service::{add_quantities_to_item, validate_quantity_item, validate_requested_ids};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{MergeInventoryItems as MergeInventoryItemsCommand, UserId};
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
    target_inventory_item_id: Uuid,
    source_inventory_item_ids: Vec<Uuid>,
}

impl TryFrom<JsonData> for MergeInventoryItemsCommand {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        Self::parse(
            value.target_inventory_item_id,
            value.source_inventory_item_ids,
        )
    }
}

#[derive(thiserror::Error)]
pub enum MergeInventoryItemsError {
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

impl std::fmt::Debug for MergeInventoryItemsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for MergeInventoryItemsError {
    fn status_code(&self) -> StatusCode {
        match self {
            MergeInventoryItemsError::ValidationError(_) => StatusCode::BAD_REQUEST,
            MergeInventoryItemsError::Forbidden(_) => StatusCode::FORBIDDEN,
            MergeInventoryItemsError::NotFound(_) => StatusCode::NOT_FOUND,
            MergeInventoryItemsError::ConflictError(_) => StatusCode::CONFLICT,
            MergeInventoryItemsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<InventoryItemDatabaseError> for MergeInventoryItemsError {
    fn from(error: InventoryItemDatabaseError) -> Self {
        match error {
            InventoryItemDatabaseError::Validation(message) => Self::ValidationError(message),
            InventoryItemDatabaseError::NotFound(message) => Self::NotFound(message),
            InventoryItemDatabaseError::Conflict(message) => Self::ConflictError(message),
            InventoryItemDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Merge inventory items",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id)
)]
pub async fn merge_inventory_items(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, MergeInventoryItemsError> {
    let command = MergeInventoryItemsCommand::try_from(payload.into_inner())
        .map_err(MergeInventoryItemsError::ValidationError)?;
    let target_inventory_item_id = Uuid::from(command.target_inventory_item_id);
    let source_inventory_item_ids: Vec<_> = command
        .source_inventory_item_ids
        .into_inner()
        .into_iter()
        .map(Uuid::from)
        .collect();

    let mut all_ids = Vec::with_capacity(source_inventory_item_ids.len() + 1);
    all_ids.push(target_inventory_item_id);
    all_ids.extend(source_inventory_item_ids.iter().copied());
    for inventory_item_id in &all_ids {
        if !validate_permission(
            &pool,
            &actor_user_id,
            ResourceType::InventoryItem,
            Action::Update(*inventory_item_id),
        )
        .await?
        {
            return Err(MergeInventoryItemsError::Forbidden(
                "You don't have permission to merge these inventory items.".into(),
            ));
        }
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let rows = fetch_inventory_items_for_update(&mut transaction, &all_ids).await?;
    validate_requested_ids(&all_ids, &rows)?;

    let target_before = find_row(&rows, target_inventory_item_id, "Target")?;
    validate_quantity_item(&target_before).map_err(MergeInventoryItemsError::ValidationError)?;
    let mut sources = Vec::with_capacity(source_inventory_item_ids.len());
    for source_id in &source_inventory_item_ids {
        let source = find_row(&rows, *source_id, "Source")?;
        validate_quantity_item(&source).map_err(MergeInventoryItemsError::ValidationError)?;
        validate_merge_compatible(&target_before, &source)?;
        sources.push(source);
    }

    let quantity_delta: f64 = sources.iter().map(|source| source.quantity_on_hand).sum();
    let allocated_delta: f64 = sources.iter().map(|source| source.quantity_allocated).sum();
    let source_ids: Vec<_> = sources
        .iter()
        .map(|source| source.inventory_item_id)
        .collect();

    let moved_attachment_ids = move_inventory_item_attachments(
        &mut transaction,
        &source_ids,
        target_before.inventory_item_id,
    )
    .await?;
    let target_after = add_quantities_to_item(
        &mut transaction,
        target_before.inventory_item_id,
        quantity_delta,
        allocated_delta,
    )
    .await?;
    for source_id in &source_ids {
        delete_inventory_item_from_database(&mut transaction, *source_id).await?;
    }

    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Adjust,
        AuditResource::InventoryItem,
        Some(target_after.inventory_item_id),
        merge_inventory_items_rollback_details(&target_before, &sources, &moved_attachment_ids),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to merge inventory items.")?;

    Ok(HttpResponse::Ok().json(InventoryItemResponse::from_row(target_after, true)))
}

fn find_row(
    rows: &[InventoryItemRow],
    inventory_item_id: Uuid,
    label: &str,
) -> Result<InventoryItemRow, MergeInventoryItemsError> {
    rows.iter()
        .find(|row| row.inventory_item_id == inventory_item_id)
        .cloned()
        .ok_or(MergeInventoryItemsError::NotFound(format!(
            "{label} inventory item not found"
        )))
}

fn validate_merge_compatible(
    target: &InventoryItemRow,
    source: &InventoryItemRow,
) -> Result<(), MergeInventoryItemsError> {
    if target.laboratory_id == source.laboratory_id
        && target.asset_id == source.asset_id
        && target.batch_number == source.batch_number
        && target.location_id == source.location_id
        && target.status == source.status
    {
        Ok(())
    } else {
        Err(MergeInventoryItemsError::ValidationError(
            "Source inventory items must match target asset, batch, location, and status".into(),
        ))
    }
}
