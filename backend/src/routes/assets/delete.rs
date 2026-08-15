use super::model::{DeletedAttachmentRow, delete_asset_rollback_details};
use super::queries::{
    AssetDatabaseError, delete_asset_attachments, delete_asset_from_database,
    fetch_asset_for_update, fetch_inventory_items_for_asset_for_update,
    fetch_parameter_values_for_asset_for_update,
};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AssetId, FileStorageKey};
use crate::file_storage::FileStorage;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum DeleteAssetError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for DeleteAssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteAssetError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteAssetError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteAssetError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteAssetError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteAssetError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<AssetDatabaseError> for DeleteAssetError {
    fn from(error: AssetDatabaseError) -> Self {
        match error {
            AssetDatabaseError::Validation(message) => Self::ConflictError(message),
            AssetDatabaseError::Conflict(message) => Self::ConflictError(message),
            AssetDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Delete an asset",
    skip(pool, storage),
    fields(actor_user_id=%laboratory_context.actor().user_id, asset_id=%asset_id)
)]
pub async fn delete_asset(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    storage: web::Data<FileStorage>,
    asset_id: AssetId,
) -> Result<HttpResponse, DeleteAssetError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::Asset,
        Action::Delete(asset_id.into()),
    )
    .await?
    {
        return Err(DeleteAssetError::Forbidden(
            "You don't have permission to delete this asset.".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let asset = fetch_asset_for_update(&mut transaction, asset_id.into())
        .await?
        .ok_or(DeleteAssetError::NotFound("Asset not found".into()))?;
    let inventory_items =
        fetch_inventory_items_for_asset_for_update(&mut transaction, asset.asset_id).await?;
    let inventory_item_ids: Vec<_> = inventory_items
        .iter()
        .map(|item| item.inventory_item_id)
        .collect();
    let parameter_values =
        fetch_parameter_values_for_asset_for_update(&mut transaction, asset.asset_id).await?;
    let deleted_attachments =
        delete_asset_attachments(&mut transaction, asset.asset_id, &inventory_item_ids).await?;
    let attachment_ids: Vec<_> = deleted_attachments
        .iter()
        .map(|attachment| attachment.attachment_id)
        .collect();
    delete_asset_from_database(&mut transaction, asset.asset_id).await?;

    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Delete,
        AuditResource::Asset,
        Some(asset.asset_id),
        delete_asset_rollback_details(&asset, &inventory_items, &parameter_values, &attachment_ids),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete an asset.")?;
    delete_storage_objects(&storage, &deleted_attachments).await?;

    Ok(HttpResponse::NoContent().finish())
}

async fn delete_storage_objects(
    storage: &FileStorage,
    attachments: &[DeletedAttachmentRow],
) -> Result<(), anyhow::Error> {
    for attachment in attachments {
        let storage_key =
            FileStorageKey::parse(attachment.storage_key.clone()).map_err(anyhow::Error::msg)?;
        storage.delete(&storage_key).await?;
    }

    Ok(())
}
