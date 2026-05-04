use super::model::Attachment;
use crate::authentication::Actor;
use crate::utils::ApiError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(super) fn attachment_select() -> &'static str {
    r#"
    SELECT
        attachments.attachment_id,
        attachments.laboratory_id,
        laboratories.name AS laboratory_name,
        attachments.resource_type,
        attachments.resource_id,
        attachments.file_name,
        attachments.mime_type,
        attachments.file_size_bytes,
        attachments.storage_url,
        attachments.visibility,
        attachments.uploaded_by_user_id,
        users.username AS uploaded_by_username,
        attachments.created_at
    FROM attachments
    INNER JOIN laboratories USING (laboratory_id)
    LEFT JOIN users ON users.user_id = attachments.uploaded_by_user_id
    "#
}

pub(super) async fn fetch_attachment(
    pool: &PgPool,
    attachment_id: Uuid,
) -> Result<Attachment, ApiError> {
    let query = format!(
        "{} WHERE attachments.attachment_id = $1",
        attachment_select()
    );
    sqlx::query_as::<_, Attachment>(&query)
        .bind(attachment_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound)
}

pub(super) async fn fetch_attachment_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    attachment_id: Uuid,
) -> Result<Attachment, ApiError> {
    let query = format!(
        "{} WHERE attachments.attachment_id = $1",
        attachment_select()
    );
    sqlx::query_as::<_, Attachment>(&query)
        .bind(attachment_id)
        .fetch_optional(transaction.as_mut())
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound)
}

pub(super) fn normalize_resource_type(resource_type: &str) -> Result<&'static str, ApiError> {
    match resource_type.trim() {
        "asset" => Ok("asset"),
        "inventory_item" => Ok("inventory_item"),
        "maintenance_record" => Ok("maintenance_record"),
        "borrow_request" => Ok("borrow_request"),
        _ => Err(ApiError::BadRequest(
            "Unknown attachment resource_type".into(),
        )),
    }
}

pub(super) fn normalize_visibility(visibility: Option<&str>) -> Result<&'static str, ApiError> {
    match visibility.map(str::trim).unwrap_or("internal") {
        "public" => Ok("public"),
        "internal" => Ok("internal"),
        _ => Err(ApiError::BadRequest("Unknown attachment visibility".into())),
    }
}

pub(super) async fn resolve_resource_laboratory(
    pool: &PgPool,
    actor: &Actor,
    resource_type: &str,
    resource_id: Uuid,
) -> Result<Uuid, ApiError> {
    match resource_type {
        "asset" => sqlx::query_scalar("SELECT laboratory_id FROM assets WHERE asset_id = $1")
            .bind(resource_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?
            .ok_or(ApiError::NotFound),
        "inventory_item" => sqlx::query_scalar(
            "SELECT laboratory_id FROM asset_inventory_items WHERE inventory_item_id = $1",
        )
        .bind(resource_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound),
        "maintenance_record" => sqlx::query_scalar(
            "SELECT laboratory_id FROM maintenance_records WHERE maintenance_record_id = $1",
        )
        .bind(resource_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound),
        "borrow_request" => {
            let row: Option<(Uuid, Uuid)> = sqlx::query_as(
                "SELECT requester_laboratory_id, owner_laboratory_id FROM borrow_requests WHERE borrow_request_id = $1",
            )
            .bind(resource_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?;
            match row {
                Some((requester_laboratory_id, owner_laboratory_id)) => {
                    if actor.is_owner() {
                        Ok(owner_laboratory_id)
                    } else if actor.laboratory_id == Some(requester_laboratory_id) {
                        Ok(requester_laboratory_id)
                    } else if actor.laboratory_id == Some(owner_laboratory_id) {
                        Ok(owner_laboratory_id)
                    } else {
                        Err(ApiError::Forbidden)
                    }
                }
                None => Err(ApiError::NotFound),
            }
        }
        _ => Err(ApiError::BadRequest(
            "Unknown attachment resource_type".into(),
        )),
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

pub(super) fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error
        && let Some("23514") = database_error.code().as_deref()
    {
        return ApiError::BadRequest("Invalid attachment data".into());
    }
    ApiError::UnexpectedError(error.into())
}
