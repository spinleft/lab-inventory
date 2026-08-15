use super::model::delete_asset_parameter_rollback_details;
use super::queries::{
    AssetParameterDatabaseError, delete_asset_parameter_from_database,
    fetch_asset_parameter_for_update, fetch_asset_parameter_options_for_update,
};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::AssetParameterId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum DeleteAssetParameterError {
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

impl std::fmt::Debug for DeleteAssetParameterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteAssetParameterError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteAssetParameterError::ValidationError(_) => StatusCode::BAD_REQUEST,
            DeleteAssetParameterError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteAssetParameterError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteAssetParameterError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteAssetParameterError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<AssetParameterDatabaseError> for DeleteAssetParameterError {
    fn from(error: AssetParameterDatabaseError) -> Self {
        match error {
            AssetParameterDatabaseError::Validation(message) => Self::ValidationError(message),
            AssetParameterDatabaseError::Conflict(message) => Self::ConflictError(message),
            AssetParameterDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Delete an asset parameter",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, parameter_id=%parameter_id)
)]
pub async fn delete_asset_parameter(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    parameter_id: AssetParameterId,
) -> Result<HttpResponse, DeleteAssetParameterError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::AssetParameter,
        Action::Delete(parameter_id.into()),
    )
    .await?
    {
        return Err(DeleteAssetParameterError::Forbidden(
            "You are not allowed to delete this asset parameter.".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_asset_parameter_for_update(&mut transaction, parameter_id)
        .await?
        .ok_or(DeleteAssetParameterError::NotFound(
            "Asset parameter not found".into(),
        ))?;

    let options =
        fetch_asset_parameter_options_for_update(&mut transaction, existing.parameter_type_id)
            .await?;
    delete_asset_parameter_from_database(&mut transaction, existing.parameter_type_id).await?;
    record_audit(
        &mut transaction,
        laboratory_context.actor(),
        AuditAction::Delete,
        AuditResource::AssetParameter,
        Some(existing.parameter_type_id),
        delete_asset_parameter_rollback_details(&existing, &options),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete an asset parameter.")?;

    Ok(HttpResponse::NoContent().finish())
}
