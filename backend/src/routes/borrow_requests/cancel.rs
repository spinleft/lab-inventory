use super::helpers::{
    InventoryTransactionData, ensure_can_cancel, fetch_borrow_request_for_update,
    fetch_borrow_request_in_transaction, fetch_inventory_item_for_update,
    record_inventory_transaction, update_inventory_after_returned,
};
use super::model::{APPROVED, PENDING};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::idempotency::{NextAction, idempotency_key_from_request, save_response, try_processing};
use crate::utils::ApiError;
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    reason: Option<String>,
}

#[tracing::instrument(name = "Cancel a borrow request", skip(request, pool, payload), fields(user_id=%user_id, borrow_request_id=%borrow_request_id))]
pub async fn cancel_borrow_request(
    request: HttpRequest,
    user_id: UserId,
    pool: web::Data<PgPool>,
    borrow_request_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let idempotency_key = idempotency_key_from_request(&request)?;
    let borrow_request_id = borrow_request_id.into_inner();

    match try_processing(pool.get_ref(), &idempotency_key, *user_id).await? {
        NextAction::ReturnSavedResponse(response) => Ok(response),
        NextAction::StartProcessing(mut transaction) => {
            let borrow_request =
                fetch_borrow_request_for_update(&mut transaction, borrow_request_id).await?;
            ensure_can_cancel(&actor, &borrow_request)?;
            if !matches!(borrow_request.status.as_str(), PENDING | APPROVED) {
                return Err(ApiError::Conflict(
                    "Borrow request cannot be cancelled".into(),
                ));
            }
            let item =
                fetch_inventory_item_for_update(&mut transaction, borrow_request.inventory_item_id)
                    .await?;
            if borrow_request.status == APPROVED {
                update_inventory_after_returned(
                    &mut transaction,
                    &item,
                    borrow_request.requested_quantity,
                )
                .await?;
            }
            sqlx::query(
                r#"
                UPDATE borrow_requests
                SET status = 'cancelled',
                    cancelled_by_user_id = $2,
                    cancelled_at = now(),
                    updated_at = now()
                WHERE borrow_request_id = $1
                "#,
            )
            .bind(borrow_request.borrow_request_id)
            .bind(actor.user_id)
            .execute(transaction.as_mut())
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?;

            let allocated_delta = if borrow_request.status == APPROVED {
                -borrow_request.requested_quantity
            } else {
                0.0
            };
            record_inventory_transaction(
                &mut transaction,
                InventoryTransactionData {
                    inventory_item_id: item.inventory_item_id,
                    laboratory_id: item.laboratory_id,
                    actor_user_id: actor.user_id,
                    actor_laboratory_id: actor.laboratory_id,
                    action: if borrow_request.status == APPROVED {
                        AuditAction::ReleaseAllocation
                    } else {
                        AuditAction::Update
                    },
                    quantity_delta: 0.0,
                    allocated_delta,
                    from_location_id: item.location_id,
                    to_location_id: item.location_id,
                    borrow_request_id: borrow_request.borrow_request_id,
                    details: json!({ "status": "cancelled", "reason": payload.reason }),
                },
            )
            .await?;
            record_audit(
                &mut transaction,
                &actor,
                Some(borrow_request.owner_laboratory_id),
                AuditAction::Cancel,
                AuditResource::BorrowRequest,
                Some(borrow_request.borrow_request_id),
                json!({ "reason": payload.reason }),
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
