use super::model::{
    BorrowRequestError, BorrowRequestResponse, BorrowRequestStatus, actor_for_user,
    fetch_borrow_request_for_update, fetch_user_snapshot, record_borrow_request_audit,
    validate_resolver_actor,
};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{LaboratoryId, UserId};
use crate::routes::inventory_items::{
    fetch_inventory_item_for_update, update_inventory_item_rollback_details,
};
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveBorrowRequestBody {
    decision: String,
    decision_note: Option<String>,
}

#[tracing::instrument(
    name = "Resolve borrow request",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id)
)]
pub async fn resolve_borrow_request(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    path: web::Path<(Uuid, Uuid)>,
    payload: web::Json<ResolveBorrowRequestBody>,
) -> Result<HttpResponse, BorrowRequestError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let (laboratory_id, borrow_request_id) = path.into_inner();
    let laboratory_id = LaboratoryId::parse(laboratory_id)
        .map_err(|e| BorrowRequestError::UnexpectedError(anyhow::anyhow!(e)))?;
    validate_resolver_actor(&actor, laboratory_id)?;
    let decision = BorrowRequestStatus::parse(&payload.decision)
        .map_err(BorrowRequestError::ValidationError)?;
    if decision == BorrowRequestStatus::Pending {
        return Err(BorrowRequestError::ValidationError(
            "decision must be approved or rejected".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let request =
        fetch_borrow_request_for_update(&mut transaction, *laboratory_id, borrow_request_id)
            .await?
            .ok_or_else(|| BorrowRequestError::NotFound("Borrow request not found".into()))?;
    if request.status != BorrowRequestStatus::Pending.as_str() {
        return Err(BorrowRequestError::ConflictError(
            "Borrow request has already been resolved".into(),
        ));
    }

    let item = fetch_inventory_item_for_update(&mut transaction, request.inventory_item_id)
        .await?
        .ok_or_else(|| BorrowRequestError::NotFound("Inventory item not found".into()))?;
    if decision == BorrowRequestStatus::Approved && item.status != "available" {
        return Err(BorrowRequestError::ConflictError(
            "Only available inventory items can be approved for borrowing".into(),
        ));
    }

    let decision_note = payload
        .decision_note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let (reviewer_username, reviewer_user_type) =
        fetch_user_snapshot(&mut transaction, actor.user_id).await?;

    if decision == BorrowRequestStatus::Approved {
        sqlx::query(
            r#"
            UPDATE asset_inventory_items
            SET status = 'borrowed',
                updated_at = now()
            WHERE inventory_item_id = $1
            "#,
        )
        .bind(item.inventory_item_id)
        .execute(transaction.as_mut())
        .await
        .map_err(|e| BorrowRequestError::UnexpectedError(e.into()))?;
        record_audit(
            &mut transaction,
            &actor,
            AuditAction::Update,
            AuditResource::InventoryItem,
            Some(item.inventory_item_id),
            update_inventory_item_rollback_details(&item),
        )
        .await?;
    }

    sqlx::query(
        r#"
        UPDATE federation_borrow_requests
        SET status = $3,
            reviewed_by_user_id = $4,
            reviewed_by_username = $5,
            reviewed_by_user_type = $6,
            reviewed_at = now(),
            decision_note = $7,
            updated_at = now()
        WHERE borrow_request_id = $1
          AND local_laboratory_id = $2
        "#,
    )
    .bind(borrow_request_id)
    .bind(*laboratory_id)
    .bind(decision.as_str())
    .bind(*actor.user_id)
    .bind(reviewer_username)
    .bind(reviewer_user_type)
    .bind(decision_note)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| BorrowRequestError::UnexpectedError(e.into()))?;

    let updated =
        fetch_borrow_request_for_update(&mut transaction, *laboratory_id, borrow_request_id)
            .await?
            .ok_or_else(|| BorrowRequestError::NotFound("Borrow request not found".into()))?;

    record_borrow_request_audit(
        &mut transaction,
        &actor,
        AuditAction::Update,
        updated.borrow_request_id,
        updated.inventory_item_id,
        updated.status.as_str(),
        updated.decision_note.as_deref(),
    )
    .await?;

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to resolve a borrow request.")?;

    Ok(HttpResponse::Ok().json(BorrowRequestResponse::from(updated)))
}
