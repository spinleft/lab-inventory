use super::helpers::{fetch_laboratory, map_database_error, required_text};
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
    name: Option<String>,
    address: Option<String>,
    description: Option<Option<String>>,
    contact: Option<Option<String>>,
}

#[tracing::instrument(
    name = "Update a laboratory",
    skip(pool, payload),
    fields(user_id=%user_id, laboratory_id=%laboratory_id)
)]
pub async fn update_laboratory(
    user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    if !actor.is_owner() && !actor.is_maintainer() {
        return Err(ApiError::Forbidden);
    }

    let existing = fetch_laboratory(pool.get_ref(), *laboratory_id).await?;
    if actor.is_maintainer() && actor.laboratory_id != Some(existing.laboratory_id) {
        return Err(ApiError::Forbidden);
    }

    let name = payload
        .name
        .as_deref()
        .map(|name| required_text(name, "name"))
        .transpose()?;
    let address = payload
        .address
        .as_deref()
        .map(|address| required_text(address, "address"))
        .transpose()?;
    let should_update_description = payload.description.is_some();
    let description = payload
        .description
        .as_ref()
        .and_then(|value| value.as_deref());
    let should_update_contact = payload.contact.is_some();
    let contact = payload.contact.as_ref().and_then(|value| value.as_deref());

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    let laboratory = sqlx::query_as::<_, Laboratory>(
        r#"
        UPDATE laboratories
        SET
            name = COALESCE($2, name),
            address = COALESCE($3, address),
            description = CASE WHEN $4 THEN $5 ELSE description END,
            contact = CASE WHEN $6 THEN $7 ELSE contact END,
            updated_at = now()
        WHERE laboratory_id = $1
        RETURNING laboratory_id, name, address, description, contact, created_at, updated_at
        "#,
    )
    .bind(existing.laboratory_id)
    .bind(name)
    .bind(address)
    .bind(should_update_description)
    .bind(description)
    .bind(should_update_contact)
    .bind(contact)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        Some(laboratory.laboratory_id),
        AuditAction::Update,
        AuditResource::Laboratory,
        Some(laboratory.laboratory_id),
        json!({ "name": laboratory.name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(laboratory))
}
