use super::model::fetch_user;
use super::validation::{map_database_error, validate_user_management};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(
    name = "Delete a user",
    skip(pool),
    fields(actor_user_id=%user_id, target_user_id=%target_user_id)
)]
pub async fn delete_user(
    user_id: UserId,
    pool: web::Data<PgPool>,
    target_user_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let target = fetch_user(pool.get_ref(), *target_user_id).await?;
    if target.user_id == actor.user_id {
        return Err(ApiError::BadRequest(
            "Users cannot delete themselves".into(),
        ));
    }
    validate_user_management(
        pool.get_ref(),
        &actor,
        &target.user_type_name,
        target.laboratory_id,
    )
    .await?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    sqlx::query("DELETE FROM users WHERE user_id = $1")
        .bind(target.user_id)
        .execute(transaction.as_mut())
        .await
        .map_err(map_database_error)?;

    record_audit(
        &mut transaction,
        &actor,
        target.laboratory_id,
        AuditAction::Delete,
        AuditResource::User,
        Some(target.user_id),
        json!({ "username": target.username, "user_type": target.user_type_name }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::NoContent().finish())
}
