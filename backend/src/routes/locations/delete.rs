use super::helpers::{ensure_can_write, fetch_location, map_database_error};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Delete a location", skip(pool), fields(user_id=%user_id, location_id=%location_id))]
pub async fn delete_location(
    user_id: UserId,
    pool: web::Data<PgPool>,
    location_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let location_id = location_id.into_inner();
    let location = fetch_location(pool.get_ref(), location_id).await?;
    ensure_can_write(&actor, location.laboratory_id)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    sqlx::query("DELETE FROM locations WHERE location_id = $1")
        .bind(location_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
    record_audit(
        &mut transaction,
        &actor,
        Some(location.laboratory_id),
        AuditAction::Delete,
        AuditResource::Location,
        Some(location.location_id),
        json!({ "name": location.name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::NoContent().finish())
}
