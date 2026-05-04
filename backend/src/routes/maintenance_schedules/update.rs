use super::helpers::{
    ensure_can_write, fetch_maintenance_schedule, fetch_maintenance_schedule_in_transaction,
    map_database_error, required_text, validate_schedule_numbers,
};
use super::model::MaintenanceScheduleResponse;
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    schedule_name: Option<String>,
    interval_days: Option<i32>,
    next_maintenance_at: Option<DateTime<Utc>>,
    remind_before_days: Option<i32>,
    is_active: Option<bool>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
}

#[tracing::instrument(name = "Update a maintenance schedule", skip(pool, payload), fields(user_id=%user_id, maintenance_schedule_id=%maintenance_schedule_id))]
pub async fn update_maintenance_schedule(
    user_id: UserId,
    pool: web::Data<PgPool>,
    maintenance_schedule_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let maintenance_schedule_id = maintenance_schedule_id.into_inner();
    let existing = fetch_maintenance_schedule(pool.get_ref(), maintenance_schedule_id).await?;
    ensure_can_write(&actor, existing.laboratory_id)?;
    let interval_days = payload.interval_days.unwrap_or(existing.interval_days);
    let remind_before_days = payload
        .remind_before_days
        .unwrap_or(existing.remind_before_days);
    validate_schedule_numbers(interval_days, remind_before_days)?;
    let schedule_name = match payload.schedule_name.as_deref() {
        Some(schedule_name) => Some(required_text(schedule_name, "schedule_name")?),
        None => None,
    };

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    sqlx::query(
        r#"
        UPDATE maintenance_schedules
        SET
            schedule_name = COALESCE($2, schedule_name),
            interval_days = COALESCE($3, interval_days),
            next_maintenance_at = COALESCE($4, next_maintenance_at),
            remind_before_days = COALESCE($5, remind_before_days),
            is_active = COALESCE($6, is_active),
            public_notes = COALESCE($7, public_notes),
            internal_notes = COALESCE($8, internal_notes),
            updated_at = now()
        WHERE maintenance_schedule_id = $1
        "#,
    )
    .bind(maintenance_schedule_id)
    .bind(schedule_name)
    .bind(payload.interval_days)
    .bind(payload.next_maintenance_at)
    .bind(payload.remind_before_days)
    .bind(payload.is_active)
    .bind(payload.public_notes.as_deref())
    .bind(payload.internal_notes.as_deref())
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(existing.laboratory_id),
        AuditAction::Update,
        AuditResource::MaintenanceSchedule,
        Some(maintenance_schedule_id),
        json!({ "maintenance_schedule_id": maintenance_schedule_id }),
    )
    .await?;
    let schedule =
        fetch_maintenance_schedule_in_transaction(&mut transaction, maintenance_schedule_id)
            .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(MaintenanceScheduleResponse::from_row(schedule, &actor)))
}
