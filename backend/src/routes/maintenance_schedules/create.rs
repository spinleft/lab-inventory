use super::helpers::{
    ensure_can_write, fetch_maintenance_schedule_in_transaction, map_database_error, required_text,
    resolve_target_laboratory, validate_schedule_numbers,
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
    asset_id: Option<Uuid>,
    inventory_item_id: Option<Uuid>,
    schedule_name: String,
    interval_days: i32,
    next_maintenance_at: DateTime<Utc>,
    remind_before_days: Option<i32>,
    is_active: Option<bool>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
}

#[tracing::instrument(name = "Create a maintenance schedule", skip(pool, payload), fields(user_id=%user_id))]
pub async fn create_maintenance_schedule(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let laboratory_id =
        resolve_target_laboratory(pool.get_ref(), payload.asset_id, payload.inventory_item_id)
            .await?;
    ensure_can_write(&actor, laboratory_id)?;
    let schedule_name = required_text(&payload.schedule_name, "schedule_name")?;
    let remind_before_days = payload.remind_before_days.unwrap_or(7);
    validate_schedule_numbers(payload.interval_days, remind_before_days)?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let maintenance_schedule_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO maintenance_schedules (
            maintenance_schedule_id,
            asset_id,
            inventory_item_id,
            laboratory_id,
            schedule_name,
            interval_days,
            next_maintenance_at,
            remind_before_days,
            is_active,
            public_notes,
            internal_notes,
            created_by_user_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, COALESCE($9, true), $10, $11, $12)
        "#,
    )
    .bind(maintenance_schedule_id)
    .bind(payload.asset_id)
    .bind(payload.inventory_item_id)
    .bind(laboratory_id)
    .bind(schedule_name)
    .bind(payload.interval_days)
    .bind(payload.next_maintenance_at)
    .bind(remind_before_days)
    .bind(payload.is_active)
    .bind(payload.public_notes.as_deref())
    .bind(payload.internal_notes.as_deref())
    .bind(actor.user_id)
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(laboratory_id),
        AuditAction::Create,
        AuditResource::MaintenanceSchedule,
        Some(maintenance_schedule_id),
        json!({ "schedule_name": schedule_name }),
    )
    .await?;
    let schedule =
        fetch_maintenance_schedule_in_transaction(&mut transaction, maintenance_schedule_id)
            .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(MaintenanceScheduleResponse::from_row(schedule, &actor)))
}
