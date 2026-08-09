use super::model::{
    AssetDatabaseError, DeletedAttachmentRow, delete_asset_attachments,
    delete_asset_rollback_details, fetch_asset_for_update,
    fetch_inventory_items_for_asset_for_update, fetch_parameter_values_for_asset_for_update,
    map_database_error,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AssetId, FileStorageKey, UserId};
use crate::file_storage::FileStorage;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

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
    fields(actor_user_id=%actor_user_id, asset_id=%asset_id)
)]
pub async fn delete_asset(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<FileStorage>,
    asset_id: web::Path<Uuid>,
) -> Result<HttpResponse, DeleteAssetError> {
    let asset_id: AssetId = asset_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
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
        actor_user_id,
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

#[tracing::instrument(
    name = "Deleting asset from the database",
    skip(transaction),
    fields(asset_id=%asset_id)
)]
async fn delete_asset_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
) -> Result<(), DeleteAssetError> {
    sqlx::query!(
        r#"
        DELETE FROM assets
        WHERE asset_id = $1
        "#,
        asset_id,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(|e| DeleteAssetError::from(map_database_error(e)))?;

    Ok(())
}
