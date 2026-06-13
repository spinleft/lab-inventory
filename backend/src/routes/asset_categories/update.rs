use super::helpers::{
    ensure_can_write, fetch_asset_category, map_database_error, required_text,
    validate_parent_category,
};
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
    parent_category_id: Option<Option<Uuid>>,
    name: Option<String>,
    description: Option<String>,
}

#[tracing::instrument(name = "Update an asset category", skip(pool, payload), fields(user_id=%user_id, category_id=%category_id))]
pub async fn update_asset_category(
    user_id: UserId,
    pool: web::Data<PgPool>,
    category_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let category_id = category_id.into_inner();
    let existing = fetch_asset_category(pool.get_ref(), category_id).await?;
    ensure_can_write(&actor, existing.laboratory_id)?;
    if let Some(parent_category_id) = payload.parent_category_id {
        validate_parent_category(
            pool.get_ref(),
            existing.laboratory_id,
            Some(category_id),
            parent_category_id,
        )
        .await?;
    }

    let name = match payload.name.as_deref() {
        Some(name) => Some(required_text(name, "name")?),
        None => None,
    };
    let update_parent = payload.parent_category_id.is_some();
    let parent_category_id = payload.parent_category_id.flatten();
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    sqlx::query(
        r#"
        UPDATE asset_categories
        SET
            parent_category_id = CASE WHEN $2 THEN $3 ELSE parent_category_id END,
            name = COALESCE($4, name),
            description = COALESCE($5, description),
            updated_at = now()
        WHERE category_id = $1
        "#,
    )
    .bind(category_id)
    .bind(update_parent)
    .bind(parent_category_id)
    .bind(name)
    .bind(payload.description.as_deref())
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(existing.laboratory_id),
        AuditAction::Update,
        AuditResource::AssetCategory,
        Some(category_id),
        json!({ "name": name.unwrap_or(&existing.name) }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    let category = fetch_asset_category(pool.get_ref(), category_id).await?;
    Ok(HttpResponse::Ok().json(category))
}
