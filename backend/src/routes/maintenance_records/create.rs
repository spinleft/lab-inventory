use super::helpers::{
    ensure_can_write, fetch_maintenance_record_in_transaction, map_database_error, required_text,
    resolve_target_laboratory, validate_responsible_user,
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
    asset_id: Option<Uuid>,
    inventory_item_id: Option<Uuid>,
    maintenance_type: String,
    maintained_at: DateTime<Utc>,
    responsible_user_id: Option<Uuid>,
    description: String,
    public_notes: Option<String>,
    internal_notes: Option<String>,
}

#[tracing::instrument(name = "Create a maintenance record", skip(pool, payload), fields(user_id=%user_id))]
pub async fn create_maintenance_record(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let laboratory_id =
        resolve_target_laboratory(pool.get_ref(), payload.asset_id, payload.inventory_item_id)
            .await?;
    ensure_can_write(&actor, laboratory_id)?;
    let maintenance_type = required_text(&payload.maintenance_type, "maintenance_type")?;
    let description = required_text(&payload.description, "description")?;
    validate_responsible_user(pool.get_ref(), laboratory_id, payload.responsible_user_id).await?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let maintenance_record_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO maintenance_records (
            maintenance_record_id,
            asset_id,
            inventory_item_id,
            laboratory_id,
            maintenance_type,
            maintained_at,
            responsible_user_id,
            description,
            public_notes,
            internal_notes,
            created_by_user_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(maintenance_record_id)
    .bind(payload.asset_id)
    .bind(payload.inventory_item_id)
    .bind(laboratory_id)
    .bind(maintenance_type)
    .bind(payload.maintained_at)
    .bind(payload.responsible_user_id)
    .bind(description)
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
        AuditResource::MaintenanceRecord,
        Some(maintenance_record_id),
        json!({ "maintenance_type": maintenance_type }),
    )
    .await?;
    let record =
        fetch_maintenance_record_in_transaction(&mut transaction, maintenance_record_id).await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(MaintenanceRecordResponse::from_row(record, &actor)))
}
