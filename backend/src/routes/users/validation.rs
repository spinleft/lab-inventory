use super::model::UserRow;
use crate::authentication::{ADMIN, Actor, USER, requires_laboratory, user_type_exists};
use crate::utils::ApiError;
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;
use uuid::Uuid;

pub(super) async fn validate_user_management(
    pool: &PgPool,
    actor: &Actor,
    target_user_type: &str,
    target_laboratory_id: Option<Uuid>,
) -> Result<(), ApiError> {
    if !user_type_exists(pool, target_user_type).await? {
        return Err(ApiError::BadRequest("Unknown user type".into()));
    }
    if requires_laboratory(target_user_type) && target_laboratory_id.is_none() {
        return Err(ApiError::BadRequest("laboratory_id is required".into()));
    }
    if !actor.can_manage_user(target_user_type, target_laboratory_id) {
        return Err(ApiError::Forbidden);
    }
    if let Some(laboratory_id) = target_laboratory_id {
        ensure_laboratory_exists(pool, laboratory_id).await?;
    }
    Ok(())
}

pub(super) fn resolve_target_laboratory(
    actor: &Actor,
    target_user_type: &str,
    requested_laboratory_id: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
    if matches!(target_user_type, ADMIN | USER) {
        return Ok(requested_laboratory_id.or(actor.laboratory_id));
    }
    Ok(requested_laboratory_id)
}

pub(super) fn ensure_can_view_user(actor: &Actor, target: &UserRow) -> Result<(), ApiError> {
    if actor.is_owner()
        || actor.user_id == target.user_id
        || (actor.is_maintainer()
            && actor.laboratory_id.is_some()
            && actor.laboratory_id == target.laboratory_id)
    {
        return Ok(());
    }
    Err(ApiError::Forbidden)
}

async fn ensure_laboratory_exists(pool: &PgPool, laboratory_id: Uuid) -> Result<(), ApiError> {
    let exists: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM laboratories WHERE laboratory_id = $1")
            .bind(laboratory_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    if exists.is_some() {
        Ok(())
    } else {
        Err(ApiError::BadRequest("Unknown laboratory".into()))
    }
}

pub(super) fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::BadRequest(format!("{field} is required")));
    }
    Ok(trimmed)
}

pub(super) fn required_secret_text(value: &Secret<String>, field: &str) -> Result<(), ApiError> {
    if value.expose_secret().trim().is_empty() {
        return Err(ApiError::BadRequest(format!("{field} is required")));
    }
    Ok(())
}

pub(super) fn normalize_user_type(user_type: &str) -> Result<String, ApiError> {
    let user_type = required_text(user_type, "user_type")?;
    if matches!(user_type, ADMIN | USER) {
        Ok(user_type.to_string())
    } else {
        Err(ApiError::BadRequest("Unknown user type".into()))
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => return ApiError::Conflict("User already exists".into()),
            Some("23503") => return ApiError::BadRequest("Invalid referenced record".into()),
            _ => {}
        }
    }
    ApiError::UnexpectedError(error.into())
}
