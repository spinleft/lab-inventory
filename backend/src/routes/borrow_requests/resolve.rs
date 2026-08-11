use super::model::{BorrowRequestResponse, BorrowRequestStatus};
use super::queries::{
    fetch_borrow_request_for_update, mark_inventory_item_borrowed, update_borrow_request_decision,
};
use super::service::{
    BorrowRequestError, fetch_user_snapshot, record_borrow_request_audit, validate_resolver_actor,
};
use crate::access_control::{BorrowRequestPathId, LaboratoryContext};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::routes::inventory_items::{
    fetch_inventory_item_for_update, update_inventory_item_rollback_details,
};
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveBorrowRequestBody {
    decision: String,
    decision_note: Option<String>,
}

#[tracing::instrument(
    name = "Resolve borrow request",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id)
)]
pub async fn resolve_borrow_request(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    borrow_request_id: BorrowRequestPathId,
    payload: web::Json<ResolveBorrowRequestBody>,
) -> Result<HttpResponse, BorrowRequestError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    let borrow_request_id = borrow_request_id.into_inner();
    validate_resolver_actor(actor, laboratory_id)?;
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
        mark_inventory_item_borrowed(&mut transaction, item.inventory_item_id).await?;
        record_audit(
            &mut transaction,
            actor,
            AuditAction::Update,
            AuditResource::InventoryItem,
            Some(item.inventory_item_id),
            update_inventory_item_rollback_details(&item),
        )
        .await?;
    }

    update_borrow_request_decision(
        &mut transaction,
        borrow_request_id,
        laboratory_id,
        decision.as_str(),
        actor.user_id,
        &reviewer_username,
        &reviewer_user_type,
        decision_note.as_deref(),
    )
    .await?;

    let updated =
        fetch_borrow_request_for_update(&mut transaction, *laboratory_id, borrow_request_id)
            .await?
            .ok_or_else(|| BorrowRequestError::NotFound("Borrow request not found".into()))?;

    record_borrow_request_audit(
        &mut transaction,
        actor,
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
