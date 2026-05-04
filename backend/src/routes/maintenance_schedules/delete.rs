use super::helpers::{ensure_can_write, fetch_maintenance_schedule, map_database_error};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Delete a maintenance schedule", skip(pool), fields(user_id=%user_id, maintenance_schedule_id=%maintenance_schedule_id))]
pub async fn delete_maintenance_schedule(
    user_id: UserId,
    pool: web::Data<PgPool>,
    maintenance_schedule_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let maintenance_schedule_id = maintenance_schedule_id.into_inner();
    let existing = fetch_maintenance_schedule(pool.get_ref(), maintenance_schedule_id).await?;
    ensure_can_write(&actor, existing.laboratory_id)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    sqlx::query("DELETE FROM maintenance_schedules WHERE maintenance_schedule_id = $1")
        .bind(maintenance_schedule_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
    record_audit(
        &mut transaction,
        &actor,
        Some(existing.laboratory_id),
        AuditAction::Delete,
        AuditResource::MaintenanceSchedule,
        Some(maintenance_schedule_id),
        json!({ "maintenance_schedule_id": maintenance_schedule_id }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::NoContent().finish())
}
