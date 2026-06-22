use super::model::{
    delete_asset_category_rollback_details, fetch_asset_category_for_update,
    fetch_asset_category_tree_for_update,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AssetCategoryId, LaboratoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum DeleteAssetCategoryError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for DeleteAssetCategoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteAssetCategoryError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteAssetCategoryError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteAssetCategoryError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteAssetCategoryError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteAssetCategoryError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Delete an asset category",
    skip(pool),
    fields(actor_user_id=%actor_user_id, category_id=%category_id)
)]
pub async fn delete_asset_category(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    category_id: web::Path<AssetCategoryId>,
) -> Result<HttpResponse, DeleteAssetCategoryError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(DeleteAssetCategoryError::UnexpectedError)?
        .ok_or(DeleteAssetCategoryError::Forbidden(
            "Actor not found in the database".into(),
        ))?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_asset_category_for_update(&mut transaction, *category_id)
        .await?
        .ok_or(DeleteAssetCategoryError::NotFound(
            "Asset category not found".into(),
        ))?;
    let laboratory_id = LaboratoryId::parse(existing.laboratory_id)
        .map_err(|e| DeleteAssetCategoryError::UnexpectedError(anyhow!(e)))?;
    validate_delete_permission(&actor, &laboratory_id)?;

    let categories = fetch_asset_category_tree_for_update(
        &mut transaction,
        laboratory_id,
        &existing.path,
    )
    .await?;
    let cleared_asset_ids =
        clear_asset_category_references(&mut transaction, laboratory_id, &existing.path)
            .await?;
    delete_asset_category_tree(&mut transaction, laboratory_id, &existing.path).await?;

    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Delete,
        AuditResource::AssetCategory,
        Some(existing.category_id),
        delete_asset_category_rollback_details(&categories, &cleared_asset_ids),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete an asset category.")?;

    Ok(HttpResponse::NoContent().finish())
}

fn validate_delete_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), DeleteAssetCategoryError> {
    if actor.can_write_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        return Err(DeleteAssetCategoryError::Forbidden(
            "You don't have permission to delete this asset category.".into(),
        ));
    }
}

#[tracing::instrument(
    name = "Clearing deleted asset category references from assets",
    skip(transaction, root_path),
    fields(laboratory_id=%laboratory_id)
)]
async fn clear_asset_category_references(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<Vec<Uuid>, DeleteAssetCategoryError> {
    let asset_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT asset_id
        FROM assets
        WHERE laboratory_id = $1
          AND category_id IN (
              SELECT category_id
              FROM asset_categories
              WHERE laboratory_id = $1
                AND path <@ $2::text::ltree
          )
        ORDER BY asset_id
        "#,
    )
    .bind(*laboratory_id)
    .bind(root_path)
    .fetch_all(transaction.as_mut())
    .await
    .map_err(|e| DeleteAssetCategoryError::UnexpectedError(e.into()))?;

    sqlx::query(
        r#"
        UPDATE assets
        SET category_id = NULL,
            updated_at = now()
        WHERE laboratory_id = $1
          AND category_id IN (
              SELECT category_id
              FROM asset_categories
              WHERE laboratory_id = $1
                AND path <@ $2::text::ltree
          )
        "#,
    )
    .bind(*laboratory_id)
    .bind(root_path)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| DeleteAssetCategoryError::UnexpectedError(e.into()))?;

    Ok(asset_ids)
}

#[tracing::instrument(
    name = "Deleting asset category tree from the database",
    skip(transaction, root_path),
    fields(laboratory_id=%laboratory_id)
)]
async fn delete_asset_category_tree(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<(), DeleteAssetCategoryError> {
    sqlx::query(
        r#"
        DELETE FROM asset_categories
        WHERE laboratory_id = $1
          AND path <@ $2::text::ltree
        "#,
    )
    .bind(*laboratory_id)
    .bind(root_path)
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

fn map_database_error(error: sqlx::Error) -> DeleteAssetCategoryError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23503") {
            return DeleteAssetCategoryError::ConflictError(
                "Asset category is referenced by other records".into(),
            );
        }
    }

    DeleteAssetCategoryError::UnexpectedError(error.into())
}
