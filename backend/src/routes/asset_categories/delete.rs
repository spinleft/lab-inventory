use super::model::delete_asset_category_rollback_details;
use super::queries::{
    AssetCategoryDatabaseError, fetch_asset_category_for_update,
    fetch_asset_category_parameter_assignments_for_categories_for_update,
    fetch_asset_category_tree_for_update,
};
use super::service::delete_asset_category_subtree;
use crate::access_control::AssetCategoryPathId;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::AssetCategoryId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum DeleteAssetCategoryError {
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

impl std::fmt::Debug for DeleteAssetCategoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteAssetCategoryError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteAssetCategoryError::ValidationError(_) => StatusCode::BAD_REQUEST,
            DeleteAssetCategoryError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteAssetCategoryError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteAssetCategoryError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteAssetCategoryError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<AssetCategoryDatabaseError> for DeleteAssetCategoryError {
    fn from(error: AssetCategoryDatabaseError) -> Self {
        match error {
            AssetCategoryDatabaseError::Validation(message) => Self::ValidationError(message),
            AssetCategoryDatabaseError::Conflict(message) => Self::ConflictError(message),
            AssetCategoryDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Delete an asset category",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, category_id=%category_id)
)]
pub async fn delete_asset_category(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    category_id: AssetCategoryPathId,
) -> Result<HttpResponse, DeleteAssetCategoryError> {
    let actor = laboratory_context.authorization_actor();
    let category_id: AssetCategoryId = category_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::AssetCategory,
        Action::Delete(category_id.into()),
    )
    .await?
    {
        return Err(DeleteAssetCategoryError::Forbidden(
            "You are not allowed to delete this asset category.".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_asset_category_for_update(&mut transaction, category_id)
        .await?
        .ok_or(DeleteAssetCategoryError::NotFound(
            "Asset category not found".into(),
        ))?;

    let categories = fetch_asset_category_tree_for_update(
        &mut transaction,
        existing.laboratory_id.into(),
        &existing.path,
    )
    .await?;
    let category_ids: Vec<_> = categories
        .iter()
        .map(|category| category.category_id)
        .collect();
    let parameter_assignments =
        fetch_asset_category_parameter_assignments_for_categories_for_update(
            &mut transaction,
            &category_ids,
        )
        .await?;
    let cleared_asset_ids = delete_asset_category_subtree(
        &mut transaction,
        existing.laboratory_id.into(),
        &existing.path,
    )
    .await?;

    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Delete,
        AuditResource::AssetCategory,
        Some(existing.category_id),
        delete_asset_category_rollback_details(
            &categories,
            &cleared_asset_ids,
            &parameter_assignments,
        ),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete an asset category.")?;

    Ok(HttpResponse::NoContent().finish())
}
