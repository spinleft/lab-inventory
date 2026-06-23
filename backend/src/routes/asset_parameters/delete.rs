use super::model::{
    delete_asset_parameter_rollback_details, fetch_asset_parameter_for_update,
    fetch_asset_parameter_options_for_update,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{AssetParameterId, LaboratoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::{Context, anyhow};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum DeleteAssetParameterError {
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
            DeleteAssetParameterError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteAssetParameterError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteAssetParameterError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteAssetParameterError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Delete an asset parameter",
    skip(pool),
    fields(actor_user_id=%actor_user_id, parameter_id=%parameter_id)
)]
pub async fn delete_asset_parameter(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    parameter_id: web::Path<AssetParameterId>,
) -> Result<HttpResponse, DeleteAssetParameterError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(DeleteAssetParameterError::UnexpectedError)?
        .ok_or(DeleteAssetParameterError::Forbidden(
            "Actor not found in the database".into(),
        ))?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_asset_parameter_for_update(&mut transaction, *parameter_id)
        .await?
        .ok_or(DeleteAssetParameterError::NotFound(
            "Asset parameter not found".into(),
        ))?;
    let laboratory_id = LaboratoryId::parse(existing.laboratory_id)
        .map_err(|e| DeleteAssetParameterError::UnexpectedError(anyhow!("{e}")))?;
    validate_delete_permission(&actor, &laboratory_id)?;

    let options =
        fetch_asset_parameter_options_for_update(&mut transaction, existing.parameter_type_id)
            .await?;
    delete_asset_parameter_from_database(&mut transaction, existing.parameter_type_id).await?;
    record_audit(
        &mut transaction,
        &actor,
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

fn validate_delete_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), DeleteAssetParameterError> {
    if actor.can_write_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(DeleteAssetParameterError::Forbidden(
            "You don't have permission to delete this asset parameter.".into(),
        ))
    }
}

#[tracing::instrument(
    name = "Deleting asset parameter from the database",
    skip(transaction),
    fields(parameter_id=%parameter_id)
)]
async fn delete_asset_parameter_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    parameter_id: Uuid,
) -> Result<(), DeleteAssetParameterError> {
    sqlx::query("DELETE FROM asset_parameter_types WHERE parameter_type_id = $1")
        .bind(parameter_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;

    Ok(())
}

fn map_database_error(error: sqlx::Error) -> DeleteAssetParameterError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23503") {
            return DeleteAssetParameterError::ConflictError(
                "Asset parameter is referenced by other records".into(),
            );
        }
    }

    DeleteAssetParameterError::UnexpectedError(error.into())
}
