use super::helpers::{ensure_can_write, fetch_asset_category, map_database_error, required_text};
use super::model::AssetCategory;
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

    let name = match payload.name.as_deref() {
        Some(name) => Some(required_text(name, "name")?),
        None => None,
    };
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let category = sqlx::query_as::<_, AssetCategory>(
        r#"
        UPDATE asset_categories
        SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            updated_at = now()
        WHERE category_id = $1
        RETURNING
            category_id,
            laboratory_id,
            (SELECT name FROM laboratories WHERE laboratory_id = asset_categories.laboratory_id) AS laboratory_name,
            name,
            description,
            created_at,
            updated_at
        "#,
    )
    .bind(category_id)
    .bind(name)
    .bind(payload.description.as_deref())
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(category.laboratory_id),
        AuditAction::Update,
        AuditResource::AssetCategory,
        Some(category.category_id),
        json!({ "name": category.name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(category))
}
