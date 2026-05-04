use super::model::Location;
use crate::authentication::Actor;
use crate::utils::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

pub(super) async fn fetch_location(pool: &PgPool, location_id: Uuid) -> Result<Location, ApiError> {
    sqlx::query_as::<_, Location>(
        r#"
        SELECT
            locations.location_id,
            locations.laboratory_id,
            laboratories.name AS laboratory_name,
            locations.parent_location_id,
            locations.name,
            locations.description,
            locations.is_active,
            locations.created_at,
            locations.updated_at
        FROM locations
        INNER JOIN laboratories USING (laboratory_id)
        WHERE locations.location_id = $1
        "#,
    )
    .bind(location_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::NotFound)
}

pub(super) fn resolve_target_laboratory(
    actor: &Actor,
    laboratory_id: Option<Uuid>,
) -> Result<Uuid, ApiError> {
    if actor.is_owner() {
        return laboratory_id
            .ok_or_else(|| ApiError::BadRequest("laboratory_id is required".into()));
    }
    let actor_laboratory_id = actor.laboratory_id.ok_or(ApiError::Forbidden)?;
    if laboratory_id.is_some() && laboratory_id != Some(actor_laboratory_id) {
        return Err(ApiError::Forbidden);
    }
    if !actor.can_write_laboratory_resource(actor_laboratory_id) {
        return Err(ApiError::Forbidden);
    }
    Ok(actor_laboratory_id)
}

pub(super) fn ensure_can_write(actor: &Actor, laboratory_id: Uuid) -> Result<(), ApiError> {
    if actor.can_write_laboratory_resource(laboratory_id) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) async fn validate_parent_location(
    pool: &PgPool,
    laboratory_id: Uuid,
    parent_location_id: Option<Uuid>,
) -> Result<(), ApiError> {
    if let Some(parent_location_id) = parent_location_id {
        let parent = fetch_location(pool, parent_location_id).await?;
        if parent.laboratory_id != laboratory_id {
            return Err(ApiError::BadRequest(
                "parent_location_id belongs to another laboratory".into(),
            ));
        }
    }
    Ok(())
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
            Some("23505") => return ApiError::Conflict("Location already exists".into()),
            Some("23503") => return ApiError::Conflict("Location is still referenced".into()),
            Some("23514") => return ApiError::BadRequest("Invalid location data".into()),
            _ => {}
        }
    }
    ApiError::UnexpectedError(error.into())
}
