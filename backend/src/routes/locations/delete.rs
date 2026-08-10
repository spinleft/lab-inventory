use super::model::delete_location_rollback_details;
use super::queries::{
    LocationDatabaseError, fetch_location_for_update, fetch_location_tree_for_update,
};
use super::service::delete_location_subtree;
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::UserId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum DeleteLocationError {
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

impl std::fmt::Debug for DeleteLocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DeleteLocationError {
    fn status_code(&self) -> StatusCode {
        match self {
            DeleteLocationError::ValidationError(_) => StatusCode::BAD_REQUEST,
            DeleteLocationError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteLocationError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteLocationError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteLocationError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<LocationDatabaseError> for DeleteLocationError {
    fn from(error: LocationDatabaseError) -> Self {
        match error {
            LocationDatabaseError::Validation(message) => Self::ValidationError(message),
            LocationDatabaseError::Conflict(message) => Self::ConflictError(message),
            LocationDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Delete a location",
    skip(pool),
    fields(actor_user_id=%actor_user_id, location_id=%location_id)
)]
pub async fn delete_location(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    location_id: web::Path<Uuid>,
) -> Result<HttpResponse, DeleteLocationError> {
    let location_id = location_id.into_inner();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::Location,
        Action::Delete(location_id),
    )
    .await?
    {
        return Err(DeleteLocationError::Forbidden(
            "You don't have permission to delete locations.".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_location_for_update(&mut transaction, location_id.into())
        .await?
        .ok_or(DeleteLocationError::NotFound("Location not found".into()))?;
    let laboratory_id = existing.laboratory_id.into();

    let locations =
        fetch_location_tree_for_update(&mut transaction, laboratory_id, &existing.path).await?;
    let cleared_inventory_item_ids =
        delete_location_subtree(&mut transaction, laboratory_id, &existing.path).await?;

    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Delete,
        AuditResource::Location,
        Some(existing.location_id),
        delete_location_rollback_details(&locations, &cleared_inventory_item_ids),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete a location.")?;

    Ok(HttpResponse::NoContent().finish())
}
