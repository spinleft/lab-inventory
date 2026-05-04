use super::model::{MaintenanceAlertResponse, MaintenanceAlertRow};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

#[tracing::instrument(name = "List maintenance alerts", skip(pool), fields(user_id=%user_id))]
pub async fn list_maintenance_alerts(
    user_id: UserId,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let alerts = sqlx::query_as::<_, MaintenanceAlertRow>(
        r#"
        SELECT
            maintenance_schedules.maintenance_schedule_id,
            CASE
                WHEN maintenance_schedules.next_maintenance_at < now() THEN 'overdue'
                ELSE 'due_soon'
            END AS alert_kind,
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
            maintenance_schedules.public_notes,
            maintenance_schedules.internal_notes
        FROM maintenance_schedules
        INNER JOIN laboratories USING (laboratory_id)
        LEFT JOIN assets AS schedule_asset ON schedule_asset.asset_id = maintenance_schedules.asset_id
        LEFT JOIN asset_inventory_items ON asset_inventory_items.inventory_item_id = maintenance_schedules.inventory_item_id
        LEFT JOIN assets AS item_asset ON item_asset.asset_id = asset_inventory_items.asset_id
        WHERE maintenance_schedules.is_active
          AND maintenance_schedules.next_maintenance_at <= now() + (maintenance_schedules.remind_before_days * interval '1 day')
        ORDER BY
            CASE WHEN maintenance_schedules.next_maintenance_at < now() THEN 0 ELSE 1 END,
            maintenance_schedules.next_maintenance_at ASC
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .into_iter()
    .map(|alert| MaintenanceAlertResponse::from_row(alert, &actor))
    .collect::<Vec<_>>();

    Ok(HttpResponse::Ok().json(alerts))
}
