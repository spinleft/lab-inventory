use super::helpers::{ensure_can_write, fetch_asset, map_database_error};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Delete an asset", skip(pool), fields(user_id=%user_id, asset_id=%asset_id))]
pub async fn delete_asset(
    user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let asset_id = asset_id.into_inner();
    let asset = fetch_asset(pool.get_ref(), asset_id).await?;
    ensure_can_write(&actor, asset.laboratory_id)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    sqlx::query("DELETE FROM assets WHERE asset_id = $1")
        .bind(asset_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
    record_audit(
        &mut transaction,
        &actor,
        Some(asset.laboratory_id),
        AuditAction::Delete,
        AuditResource::Asset,
        Some(asset.asset_id),
        json!({ "name": asset.name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::NoContent().finish())
}
