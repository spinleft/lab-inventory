use super::helpers::{ensure_can_write, fetch_asset_category, map_database_error};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Delete an asset category", skip(pool), fields(user_id=%user_id, category_id=%category_id))]
pub async fn delete_asset_category(
    user_id: UserId,
    pool: web::Data<PgPool>,
    category_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let category_id = category_id.into_inner();
    let category = fetch_asset_category(pool.get_ref(), category_id).await?;
    ensure_can_write(&actor, category.laboratory_id)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    sqlx::query("DELETE FROM asset_categories WHERE category_id = $1")
        .bind(category_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
    record_audit(
        &mut transaction,
        &actor,
        Some(category.laboratory_id),
        AuditAction::Delete,
        AuditResource::AssetCategory,
        Some(category.category_id),
        json!({ "name": category.name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::NoContent().finish())
}
