use super::helpers::{ensure_can_write, fetch_maintenance_record, map_database_error};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Delete a maintenance record", skip(pool), fields(user_id=%user_id, maintenance_record_id=%maintenance_record_id))]
pub async fn delete_maintenance_record(
    user_id: UserId,
    pool: web::Data<PgPool>,
    maintenance_record_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let maintenance_record_id = maintenance_record_id.into_inner();
    let existing = fetch_maintenance_record(pool.get_ref(), maintenance_record_id).await?;
    ensure_can_write(&actor, existing.laboratory_id)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    sqlx::query("DELETE FROM maintenance_records WHERE maintenance_record_id = $1")
        .bind(maintenance_record_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
    record_audit(
        &mut transaction,
        &actor,
        Some(existing.laboratory_id),
        AuditAction::Delete,
        AuditResource::MaintenanceRecord,
        Some(maintenance_record_id),
        json!({ "maintenance_record_id": maintenance_record_id }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::NoContent().finish())
}
