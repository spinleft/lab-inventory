use super::helpers::{map_database_error, required_text};
use super::model::Laboratory;
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
    name: String,
    address: String,
    description: Option<String>,
    contact: Option<String>,
}

#[tracing::instrument(name = "Create a laboratory", skip(pool, payload), fields(user_id=%user_id))]
pub async fn create_laboratory(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    if !actor.is_owner() {
        return Err(ApiError::Forbidden);
    }

    let name = required_text(&payload.name, "name")?;
    let address = required_text(&payload.address, "address")?;
    let laboratory_id = Uuid::new_v4();
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let laboratory = sqlx::query_as::<_, Laboratory>(
        r#"
        INSERT INTO laboratories (laboratory_id, name, address, description, contact)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING laboratory_id, name, address, description, contact, created_at, updated_at
        "#,
    )
    .bind(laboratory_id)
    .bind(name)
    .bind(address)
    .bind(payload.description.as_deref())
    .bind(payload.contact.as_deref())
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(laboratory.laboratory_id),
        AuditAction::Create,
        AuditResource::Laboratory,
        Some(laboratory.laboratory_id),
        json!({ "name": laboratory.name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(laboratory))
}
