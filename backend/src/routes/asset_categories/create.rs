use super::helpers::{
    fetch_asset_category, map_database_error, required_text, resolve_target_laboratory,
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
    laboratory_id: Option<Uuid>,
    parent_category_id: Option<Uuid>,
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
    validate_parent_category(
        pool.get_ref(),
        laboratory_id,
        None,
        payload.parent_category_id,
    )
    .await?;
    let name = required_text(&payload.name, "name")?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let category_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO asset_categories (
            category_id,
            laboratory_id,
            parent_category_id,
            name,
            description
        )
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(category_id)
    .bind(laboratory_id)
    .bind(payload.parent_category_id)
    .bind(name)
    .bind(payload.description.as_deref())
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(laboratory_id),
        AuditAction::Create,
        AuditResource::AssetCategory,
        Some(category_id),
        json!({ "name": name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    let category = fetch_asset_category(pool.get_ref(), category_id).await?;
    Ok(HttpResponse::Created().json(category))
}
