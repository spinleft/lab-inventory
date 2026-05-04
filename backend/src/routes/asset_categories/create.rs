use super::helpers::{map_database_error, required_text, resolve_target_laboratory};
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
    laboratory_id: Option<Uuid>,
    name: String,
    description: Option<String>,
}

#[tracing::instrument(name = "Create an asset category", skip(pool, payload), fields(user_id=%user_id))]
pub async fn create_asset_category(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let laboratory_id = resolve_target_laboratory(&actor, payload.laboratory_id)?;
    let name = required_text(&payload.name, "name")?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let category = sqlx::query_as::<_, AssetCategory>(
        r#"
        INSERT INTO asset_categories (category_id, laboratory_id, name, description)
        VALUES ($1, $2, $3, $4)
        RETURNING
            category_id,
            laboratory_id,
            (SELECT name FROM laboratories WHERE laboratory_id = $2) AS laboratory_name,
            name,
            description,
            created_at,
            updated_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(name)
    .bind(payload.description.as_deref())
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(category.laboratory_id),
        AuditAction::Create,
        AuditResource::AssetCategory,
        Some(category.category_id),
        json!({ "name": category.name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(category))
}
