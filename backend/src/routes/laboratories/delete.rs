use super::helpers::{fetch_laboratory, map_database_error};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(
    name = "Delete a laboratory",
    skip(pool),
    fields(user_id=%user_id, laboratory_id=%laboratory_id)
)]
pub async fn delete_laboratory(
    user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    if !actor.is_owner() {
        return Err(ApiError::Forbidden);
    }

    let existing = fetch_laboratory(pool.get_ref(), *laboratory_id).await?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    record_audit(
        &mut transaction,
        &actor,
        Some(existing.laboratory_id),
        AuditAction::Delete,
        AuditResource::Laboratory,
        Some(existing.laboratory_id),
        json!({ "name": existing.name }),
    )
    .await?;

    sqlx::query("DELETE FROM laboratories WHERE laboratory_id = $1")
        .bind(existing.laboratory_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::NoContent().finish())
}
