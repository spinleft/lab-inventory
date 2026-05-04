use super::helpers::{
    ensure_can_write, fetch_maintenance_record, fetch_maintenance_record_in_transaction,
    map_database_error, required_text, validate_responsible_user,
};
use super::model::MaintenanceRecordResponse;
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
    maintenance_type: Option<String>,
    maintained_at: Option<DateTime<Utc>>,
    responsible_user_id: Option<Uuid>,
    description: Option<String>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
}

#[tracing::instrument(name = "Update a maintenance record", skip(pool, payload), fields(user_id=%user_id, maintenance_record_id=%maintenance_record_id))]
pub async fn update_maintenance_record(
    user_id: UserId,
    pool: web::Data<PgPool>,
    maintenance_record_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let maintenance_record_id = maintenance_record_id.into_inner();
    let existing = fetch_maintenance_record(pool.get_ref(), maintenance_record_id).await?;
    ensure_can_write(&actor, existing.laboratory_id)?;
    validate_responsible_user(
        pool.get_ref(),
        existing.laboratory_id,
        payload.responsible_user_id,
    )
    .await?;
    let maintenance_type = match payload.maintenance_type.as_deref() {
        Some(maintenance_type) => Some(required_text(maintenance_type, "maintenance_type")?),
        None => None,
    };
    let description = match payload.description.as_deref() {
        Some(description) => Some(required_text(description, "description")?),
        None => None,
    };

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    sqlx::query(
        r#"
        UPDATE maintenance_records
        SET
            maintenance_type = COALESCE($2, maintenance_type),
            maintained_at = COALESCE($3, maintained_at),
            responsible_user_id = COALESCE($4, responsible_user_id),
            description = COALESCE($5, description),
            public_notes = COALESCE($6, public_notes),
            internal_notes = COALESCE($7, internal_notes),
            updated_at = now()
        WHERE maintenance_record_id = $1
        "#,
    )
    .bind(maintenance_record_id)
    .bind(maintenance_type)
    .bind(payload.maintained_at)
    .bind(payload.responsible_user_id)
    .bind(description)
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
        AuditResource::MaintenanceRecord,
        Some(maintenance_record_id),
        json!({ "maintenance_record_id": maintenance_record_id }),
    )
    .await?;
    let record =
        fetch_maintenance_record_in_transaction(&mut transaction, maintenance_record_id).await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(MaintenanceRecordResponse::from_row(record, &actor)))
}
