use super::model::AssetCategory;
use crate::authentication::Actor;
use crate::utils::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

pub(super) async fn fetch_asset_category(
    pool: &PgPool,
    category_id: Uuid,
) -> Result<AssetCategory, ApiError> {
    sqlx::query_as::<_, AssetCategory>(
        r#"
        SELECT
            asset_categories.category_id,
            asset_categories.laboratory_id,
            laboratories.name AS laboratory_name,
            asset_categories.name,
            asset_categories.description,
            asset_categories.created_at,
            asset_categories.updated_at
        FROM asset_categories
        INNER JOIN laboratories USING (laboratory_id)
        WHERE asset_categories.category_id = $1
        "#,
    )
    .bind(category_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::NotFound)
}

pub(super) fn resolve_target_laboratory(
    actor: &Actor,
    laboratory_id: Option<Uuid>,
) -> Result<Uuid, ApiError> {
    if actor.is_system_admin() {
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
            Some("23505") => return ApiError::Conflict("Asset category already exists".into()),
            Some("23503") => {
                return ApiError::Conflict("Asset category is still referenced".into());
            }
            _ => {}
        }
    }
    ApiError::UnexpectedError(error.into())
}
