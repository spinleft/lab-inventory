use super::helpers::{
    ensure_can_write, fetch_location, map_database_error, required_text, validate_parent_location,
};
use super::model::Location;
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    parent_location_id: Option<Uuid>,
    name: Option<String>,
    description: Option<String>,
    is_active: Option<bool>,
}

#[tracing::instrument(name = "Update a location", skip(pool, payload), fields(user_id=%user_id, location_id=%location_id))]
pub async fn update_location(
    user_id: UserId,
    pool: web::Data<PgPool>,
    location_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let location_id = location_id.into_inner();
    let existing = fetch_location(pool.get_ref(), location_id).await?;
    ensure_can_write(&actor, existing.laboratory_id)?;
    validate_parent_location(
        pool.get_ref(),
        existing.laboratory_id,
        payload.parent_location_id,
    )
    .await?;
    if payload.parent_location_id == Some(location_id) {
        return Err(ApiError::BadRequest(
            "parent_location_id cannot be the location itself".into(),
        ));
    }
    let name = match payload.name.as_deref() {
        Some(name) => Some(required_text(name, "name")?),
        None => None,
    };

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let location = sqlx::query_as::<_, Location>(
        r#"
        UPDATE locations
        SET
            parent_location_id = COALESCE($2, parent_location_id),
            name = COALESCE($3, name),
            description = COALESCE($4, description),
            is_active = COALESCE($5, is_active),
            updated_at = now()
        WHERE location_id = $1
        RETURNING
            location_id,
            laboratory_id,
            (SELECT name FROM laboratories WHERE laboratory_id = locations.laboratory_id) AS laboratory_name,
            parent_location_id,
            name,
            description,
            is_active,
            created_at,
            updated_at
        "#,
    )
    .bind(location_id)
    .bind(payload.parent_location_id)
    .bind(name)
    .bind(payload.description.as_deref())
    .bind(payload.is_active)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(location.laboratory_id),
        AuditAction::Update,
        AuditResource::Location,
        Some(location.location_id),
        json!({ "name": location.name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(location))
}
