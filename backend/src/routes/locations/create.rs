use super::helpers::{
    map_database_error, required_text, resolve_target_laboratory, validate_parent_location,
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
    laboratory_id: Option<Uuid>,
    parent_location_id: Option<Uuid>,
    name: String,
    description: Option<String>,
}

#[tracing::instrument(name = "Create a location", skip(pool, payload), fields(user_id=%user_id))]
pub async fn create_location(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let laboratory_id = resolve_target_laboratory(&actor, payload.laboratory_id)?;
    validate_parent_location(pool.get_ref(), laboratory_id, payload.parent_location_id).await?;
    let name = required_text(&payload.name, "name")?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let location = sqlx::query_as::<_, Location>(
        r#"
        INSERT INTO locations (location_id, laboratory_id, parent_location_id, name, description)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING
            location_id,
            laboratory_id,
            (SELECT name FROM laboratories WHERE laboratory_id = $2) AS laboratory_name,
            parent_location_id,
            name,
            description,
            is_active,
            created_at,
            updated_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(payload.parent_location_id)
    .bind(name)
    .bind(payload.description.as_deref())
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(location.laboratory_id),
        AuditAction::Create,
        AuditResource::Location,
        Some(location.location_id),
        json!({ "name": location.name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(location))
}
