use super::helpers::{
    InventoryTransactionData, ensure_can_owner_operate, fetch_borrow_request_for_update,
    fetch_borrow_request_in_transaction, fetch_inventory_item_for_update,
    record_inventory_transaction, update_inventory_after_borrowed,
};
use super::model::APPROVED;
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::idempotency::{NextAction, idempotency_key_from_request, save_response, try_processing};
use crate::utils::ApiError;
use actix_web::{HttpRequest, HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Mark a borrow request borrowed", skip(request, pool), fields(user_id=%user_id, borrow_request_id=%borrow_request_id))]
pub async fn mark_borrow_request_borrowed(
    request: HttpRequest,
    user_id: UserId,
    pool: web::Data<PgPool>,
    borrow_request_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let idempotency_key = idempotency_key_from_request(&request)?;
    let borrow_request_id = borrow_request_id.into_inner();

    match try_processing(pool.get_ref(), &idempotency_key, *user_id).await? {
        NextAction::ReturnSavedResponse(response) => Ok(response),
        NextAction::StartProcessing(mut transaction) => {
            let borrow_request =
                fetch_borrow_request_for_update(&mut transaction, borrow_request_id).await?;
            ensure_can_owner_operate(&actor, borrow_request.owner_laboratory_id)?;
            if borrow_request.status != APPROVED {
                return Err(ApiError::Conflict("Borrow request is not approved".into()));
            }
            let item =
                fetch_inventory_item_for_update(&mut transaction, borrow_request.inventory_item_id)
                    .await?;
            update_inventory_after_borrowed(&mut transaction, &item).await?;
            sqlx::query(
                r#"
                UPDATE borrow_requests
                SET status = 'borrowed',
                    borrowed_by_user_id = $2,
                    borrowed_at = now(),
                    updated_at = now()
                WHERE borrow_request_id = $1
                "#,
            )
            .bind(borrow_request.borrow_request_id)
            .bind(actor.user_id)
            .execute(transaction.as_mut())
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?;
            record_inventory_transaction(
                &mut transaction,
                InventoryTransactionData {
                    inventory_item_id: item.inventory_item_id,
                    laboratory_id: item.laboratory_id,
                    actor_user_id: actor.user_id,
                    actor_laboratory_id: actor.laboratory_id,
                    action: AuditAction::BorrowOut,
                    quantity_delta: 0.0,
                    allocated_delta: 0.0,
                    from_location_id: item.location_id,
                    to_location_id: item.location_id,
                    borrow_request_id: borrow_request.borrow_request_id,
                    details: json!({ "status": "borrowed" }),
                },
            )
            .await?;
            record_audit(
                &mut transaction,
                &actor,
                Some(borrow_request.owner_laboratory_id),
                AuditAction::BorrowOut,
                AuditResource::BorrowRequest,
                Some(borrow_request.borrow_request_id),
                json!({ "inventory_item_id": item.inventory_item_id }),
            )
            .await?;
            let response_body = fetch_borrow_request_in_transaction(
                &mut transaction,
                borrow_request.borrow_request_id,
            )
            .await?;
            let response = HttpResponse::Ok().json(response_body);
            save_response(transaction, &idempotency_key, *user_id, response).await
        }
    }
}
