use super::model::MaintenanceScheduleRow;
use crate::authentication::Actor;
use crate::utils::ApiError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(super) fn maintenance_schedule_select() -> &'static str {
    r#"
    SELECT
        maintenance_schedules.maintenance_schedule_id,
        maintenance_schedules.asset_id,
        maintenance_schedules.inventory_item_id,
        COALESCE(schedule_asset.name, item_asset.name) AS asset_name,
        COALESCE(schedule_asset.model, item_asset.model) AS asset_model,
        maintenance_schedules.laboratory_id,
        laboratories.name AS laboratory_name,
        maintenance_schedules.schedule_name,
        maintenance_schedules.interval_days,
        maintenance_schedules.next_maintenance_at,
        maintenance_schedules.remind_before_days,
        maintenance_schedules.is_active,
        maintenance_schedules.public_notes,
        maintenance_schedules.internal_notes,
        maintenance_schedules.created_by_user_id,
        maintenance_schedules.created_at,
        maintenance_schedules.updated_at
    FROM maintenance_schedules
    INNER JOIN laboratories USING (laboratory_id)
    LEFT JOIN assets AS schedule_asset ON schedule_asset.asset_id = maintenance_schedules.asset_id
    LEFT JOIN asset_inventory_items ON asset_inventory_items.inventory_item_id = maintenance_schedules.inventory_item_id
    LEFT JOIN assets AS item_asset ON item_asset.asset_id = asset_inventory_items.asset_id
    "#
}

pub(super) async fn fetch_maintenance_schedule(
    pool: &PgPool,
    maintenance_schedule_id: Uuid,
) -> Result<MaintenanceScheduleRow, ApiError> {
    let query = format!(
        "{} WHERE maintenance_schedules.maintenance_schedule_id = $1",
        maintenance_schedule_select()
    );
    sqlx::query_as::<_, MaintenanceScheduleRow>(&query)
        .bind(maintenance_schedule_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound)
}

pub(super) async fn fetch_maintenance_schedule_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    maintenance_schedule_id: Uuid,
) -> Result<MaintenanceScheduleRow, ApiError> {
    let query = format!(
        "{} WHERE maintenance_schedules.maintenance_schedule_id = $1",
        maintenance_schedule_select()
    );
    sqlx::query_as::<_, MaintenanceScheduleRow>(&query)
        .bind(maintenance_schedule_id)
        .fetch_optional(transaction.as_mut())
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound)
}

pub(super) async fn resolve_target_laboratory(
    pool: &PgPool,
    asset_id: Option<Uuid>,
    inventory_item_id: Option<Uuid>,
) -> Result<Uuid, ApiError> {
    match (asset_id, inventory_item_id) {
        (Some(_), Some(_)) | (None, None) => Err(ApiError::BadRequest(
            "exactly one of asset_id or inventory_item_id is required".into(),
        )),
        (Some(asset_id), None) => {
            sqlx::query_scalar("SELECT laboratory_id FROM assets WHERE asset_id = $1")
                .bind(asset_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ApiError::UnexpectedError(e.into()))?
                .ok_or(ApiError::NotFound)
        }
        (None, Some(inventory_item_id)) => {
            let row: Option<(Uuid, String)> = sqlx::query_as(
                "SELECT laboratory_id, tracking_mode FROM asset_inventory_items WHERE inventory_item_id = $1",
            )
            .bind(inventory_item_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?;
            match row {
                Some((laboratory_id, tracking_mode)) if tracking_mode == "serialized" => {
                    Ok(laboratory_id)
                }
                Some(_) => Err(ApiError::BadRequest(
                    "maintenance schedules for inventory items require serialized inventory".into(),
                )),
                None => Err(ApiError::NotFound),
            }
        }
    }
}

pub(super) fn ensure_can_write(actor: &Actor, laboratory_id: Uuid) -> Result<(), ApiError> {
    if actor.can_write_laboratory_resource(laboratory_id) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::BadRequest(format!("{field} is required")));
    }
    Ok(value)
}

pub(super) fn validate_schedule_numbers(
    interval_days: i32,
    remind_before_days: i32,
) -> Result<(), ApiError> {
    if interval_days <= 0 {
        return Err(ApiError::BadRequest(
            "interval_days must be positive".into(),
        ));
    }
    if remind_before_days < 0 {
        return Err(ApiError::BadRequest(
            "remind_before_days must be non-negative".into(),
        ));
    }
    Ok(())
}

pub(super) fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23503") => {
                return ApiError::Conflict("Maintenance schedule is still referenced".into());
            }
            Some("23514") => {
                return ApiError::BadRequest("Invalid maintenance schedule data".into());
            }
            _ => {}
        }
    }
    ApiError::UnexpectedError(error.into())
}
