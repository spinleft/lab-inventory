use super::model::{
    AssetModelError, delete_asset_attachments, delete_asset_rollback_details,
    fetch_asset_for_update, fetch_inventory_items_for_asset_for_update,
    fetch_parameter_values_for_asset_for_update, map_database_error,
};
use crate::access_control::{Actor, get_actor};
use crate::attachment_storage::AttachmentStorage;
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AssetId, LaboratoryId, UserId};
use crate::routes::attachments::delete_storage_objects;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use sqlx::PgPool;
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

#[tracing::instrument(
    name = "Delete an asset",
    skip(pool, storage),
    fields(actor_user_id=%actor_user_id, asset_id=%asset_id)
)]
pub async fn delete_asset(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<AttachmentStorage>,
    asset_id: web::Path<AssetId>,
) -> Result<HttpResponse, DeleteAssetError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(DeleteAssetError::UnexpectedError)?
        .ok_or(DeleteAssetError::Forbidden(
            "Actor not found in the database".into(),
        ))?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let asset = fetch_asset_for_update(&mut transaction, Uuid::from(*asset_id))
        .await?
        .ok_or(DeleteAssetError::NotFound("Asset not found".into()))?;
    let laboratory_id = LaboratoryId::parse(asset.laboratory_id)
        .map_err(|e| DeleteAssetError::UnexpectedError(anyhow!("{e}")))?;
    validate_delete_permission(&actor, &laboratory_id)?;

    let inventory_items =
        fetch_inventory_items_for_asset_for_update(&mut transaction, asset.asset_id).await?;
    let inventory_item_ids: Vec<_> = inventory_items
        .iter()
        .map(|item| item.inventory_item_id)
        .collect();
    let parameter_values =
        fetch_parameter_values_for_asset_for_update(&mut transaction, asset.asset_id).await?;
    let deleted_attachments =
        delete_asset_attachments(&mut transaction, asset.asset_id, &inventory_item_ids)
            .await
            .map_err(map_model_error)?;
    let attachment_ids = deleted_attachments
        .iter()
        .map(|row| row.attachment_id)
        .collect::<Vec<_>>();
    delete_asset_from_database(&mut transaction, asset.asset_id).await?;

    record_audit(
        &mut transaction,
        &actor,
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

fn validate_delete_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), DeleteAssetError> {
    if actor.can_write_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(DeleteAssetError::Forbidden(
            "You don't have permission to delete this asset.".into(),
        ))
    }
}

async fn delete_asset_from_database(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    asset_id: Uuid,
) -> Result<(), DeleteAssetError> {
    sqlx::query("DELETE FROM assets WHERE asset_id = $1")
        .bind(asset_id)
        .execute(transaction.as_mut())
        .await
        .map_err(|e| map_model_error(map_database_error(e)))?;
    Ok(())
}

fn map_model_error(error: AssetModelError) -> DeleteAssetError {
    match error {
        AssetModelError::Validation(message) => DeleteAssetError::ConflictError(message),
        AssetModelError::Conflict(message) => DeleteAssetError::ConflictError(message),
        AssetModelError::Unexpected(error) => DeleteAssetError::UnexpectedError(error),
    }
}
