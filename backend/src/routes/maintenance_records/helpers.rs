use super::model::MaintenanceRecordRow;
use crate::authentication::Actor;
use crate::utils::ApiError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(super) fn maintenance_record_select() -> &'static str {
    r#"
    SELECT
        maintenance_records.maintenance_record_id,
        maintenance_records.asset_id,
        maintenance_records.inventory_item_id,
        COALESCE(record_asset.name, item_asset.name) AS asset_name,
        COALESCE(record_asset.model, item_asset.model) AS asset_model,
        maintenance_records.laboratory_id,
        laboratories.name AS laboratory_name,
        maintenance_records.maintenance_type,
        maintenance_records.maintained_at,
        maintenance_records.responsible_user_id,
        responsible_user.username AS responsible_username,
        maintenance_records.description,
        maintenance_records.public_notes,
        maintenance_records.internal_notes,
        maintenance_records.created_by_user_id,
        maintenance_records.created_at,
        maintenance_records.updated_at
    FROM maintenance_records
    INNER JOIN laboratories USING (laboratory_id)
    LEFT JOIN assets AS record_asset ON record_asset.asset_id = maintenance_records.asset_id
    LEFT JOIN asset_inventory_items ON asset_inventory_items.inventory_item_id = maintenance_records.inventory_item_id
    LEFT JOIN assets AS item_asset ON item_asset.asset_id = asset_inventory_items.asset_id
    LEFT JOIN users AS responsible_user ON responsible_user.user_id = maintenance_records.responsible_user_id
    "#
}

pub(super) async fn fetch_maintenance_record(
    pool: &PgPool,
    maintenance_record_id: Uuid,
) -> Result<MaintenanceRecordRow, ApiError> {
    let query = format!(
        "{} WHERE maintenance_records.maintenance_record_id = $1",
        maintenance_record_select()
    );
    sqlx::query_as::<_, MaintenanceRecordRow>(&query)
        .bind(maintenance_record_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound)
}

pub(super) async fn fetch_maintenance_record_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    maintenance_record_id: Uuid,
) -> Result<MaintenanceRecordRow, ApiError> {
    let query = format!(
        "{} WHERE maintenance_records.maintenance_record_id = $1",
        maintenance_record_select()
    );
    sqlx::query_as::<_, MaintenanceRecordRow>(&query)
        .bind(maintenance_record_id)
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
        (None, Some(inventory_item_id)) => sqlx::query_scalar(
            "SELECT laboratory_id FROM asset_inventory_items WHERE inventory_item_id = $1",
        )
        .bind(inventory_item_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound),
    }
}

pub(super) fn ensure_can_write(actor: &Actor, laboratory_id: Uuid) -> Result<(), ApiError> {
    if actor.can_write_laboratory_resource(laboratory_id) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) async fn validate_responsible_user(
    pool: &PgPool,
    laboratory_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<(), ApiError> {
    if let Some(user_id) = user_id {
        let user_laboratory_id: Option<Option<Uuid>> =
            sqlx::query_scalar("SELECT laboratory_id FROM users WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ApiError::UnexpectedError(e.into()))?;
        match user_laboratory_id {
            Some(Some(user_laboratory_id)) if user_laboratory_id == laboratory_id => Ok(()),
            Some(None) => Ok(()),
            Some(Some(_)) => Err(ApiError::BadRequest(
                "responsible_user_id belongs to another laboratory".into(),
            )),
            None => Err(ApiError::BadRequest("Unknown responsible_user_id".into())),
        }
    } else {
        Ok(())
    }
}

pub(super) fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::BadRequest(format!("{field} is required")));
    }
    Ok(value)
}

pub(super) fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23503") => {
                return ApiError::Conflict("Maintenance record is still referenced".into());
            }
            Some("23514") => return ApiError::BadRequest("Invalid maintenance record data".into()),
            _ => {}
        }
    }
    ApiError::UnexpectedError(error.into())
}
