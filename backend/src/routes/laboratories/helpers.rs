use super::model::Laboratory;
use crate::utils::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

pub(super) async fn fetch_laboratory(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Laboratory, ApiError> {
    sqlx::query_as::<_, Laboratory>(
        r#"
        SELECT laboratory_id, name, address, description, contact, created_at, updated_at
        FROM laboratories
        WHERE laboratory_id = $1
        "#,
    )
    .bind(laboratory_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::NotFound)
}

pub(super) fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::BadRequest(format!("{field} is required")));
    }
    Ok(trimmed)
}

pub(super) fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => return ApiError::Conflict("Laboratory name already exists".into()),
            Some("23503") => return ApiError::Conflict("Laboratory is still referenced".into()),
            _ => {}
        }
    }
    ApiError::UnexpectedError(error.into())
}
