use super::model::{
    delete_location_rollback_details, fetch_location_for_update, fetch_location_tree_for_update,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{LaboratoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum DeleteLocationError {
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
            DeleteLocationError::Forbidden(_) => StatusCode::FORBIDDEN,
            DeleteLocationError::NotFound(_) => StatusCode::NOT_FOUND,
            DeleteLocationError::ConflictError(_) => StatusCode::CONFLICT,
            DeleteLocationError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
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
        Action::Delete(location_id.clone()),
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
        clear_location_references(&mut transaction, laboratory_id, &existing.path).await?;
    delete_location_tree(&mut transaction, laboratory_id, &existing.path).await?;

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

#[tracing::instrument(
    name = "Clearing deleted location references from inventory items",
    skip(transaction, root_path),
    fields(laboratory_id=%laboratory_id)
)]
async fn clear_location_references(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<Vec<Uuid>, DeleteLocationError> {
    let inventory_item_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT inventory_item_id
        FROM asset_inventory_items
        WHERE laboratory_id = $1
          AND location_id IN (
              SELECT location_id
              FROM locations
              WHERE laboratory_id = $1
                AND path <@ $2::text::ltree
          )
        ORDER BY inventory_item_id
        "#,
    )
    .bind(Uuid::from(laboratory_id))
    .bind(root_path)
    .fetch_all(transaction.as_mut())
    .await
    .map_err(|e| DeleteLocationError::UnexpectedError(e.into()))?;

    sqlx::query(
        r#"
        UPDATE asset_inventory_items
        SET location_id = NULL,
            updated_at = now()
        WHERE laboratory_id = $1
          AND location_id IN (
              SELECT location_id
              FROM locations
              WHERE laboratory_id = $1
                AND path <@ $2::text::ltree
          )
        "#,
    )
    .bind(Uuid::from(laboratory_id))
    .bind(root_path)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| DeleteLocationError::UnexpectedError(e.into()))?;

    Ok(inventory_item_ids)
}

#[tracing::instrument(
    name = "Deleting location tree from the database",
    skip(transaction, root_path),
    fields(laboratory_id=%laboratory_id)
)]
async fn delete_location_tree(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    root_path: &str,
) -> Result<(), DeleteLocationError> {
    sqlx::query(
        r#"
        DELETE FROM locations
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

fn map_database_error(error: sqlx::Error) -> DeleteLocationError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23503") {
            return DeleteLocationError::ConflictError(
                "Location is referenced by other records".into(),
            );
        }
    }

    DeleteLocationError::UnexpectedError(error.into())
}
