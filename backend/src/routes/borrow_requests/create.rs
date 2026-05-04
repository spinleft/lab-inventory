use super::helpers::{
    ensure_can_create, fetch_borrow_request_in_transaction, fetch_inventory_item_for_update,
    map_database_error, validate_positive_quantity,
};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::authentication::{UserId, get_actor};
use crate::idempotency::{NextAction, idempotency_key_from_request, save_response, try_processing};
use crate::utils::ApiError;
use actix_web::{HttpRequest, HttpResponse, web};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    inventory_item_id: Uuid,
    requested_quantity: f64,
    expected_borrowed_at: Option<DateTime<Utc>>,
    expected_returned_at: Option<DateTime<Utc>>,
    purpose: String,
}

#[tracing::instrument(name = "Create a borrow request", skip(request, pool, payload), fields(user_id=%user_id))]
pub async fn create_borrow_request(
    request: HttpRequest,
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let requester_laboratory_id = ensure_can_create(&actor)?;
    let idempotency_key = idempotency_key_from_request(&request)?;
    let purpose = payload.purpose.trim();
    if purpose.is_empty() {
        return Err(ApiError::BadRequest("purpose is required".into()));
    }
    if payload
        .expected_returned_at
        .zip(payload.expected_borrowed_at)
        .is_some_and(|(returned_at, borrowed_at)| returned_at <= borrowed_at)
    {
        return Err(ApiError::BadRequest(
            "expected_returned_at must be after expected_borrowed_at".into(),
        ));
    }

    match try_processing(pool.get_ref(), &idempotency_key, *user_id).await? {
        NextAction::ReturnSavedResponse(response) => Ok(response),
        NextAction::StartProcessing(mut transaction) => {
            let item = fetch_inventory_item_for_update(&mut transaction, payload.inventory_item_id)
                .await?;
            validate_positive_quantity(payload.requested_quantity, item.unit_allow_decimal)?;
            if item.tracking_mode == "serialized" && payload.requested_quantity != 1.0 {
                return Err(ApiError::BadRequest(
                    "serialized inventory requested_quantity must be 1".into(),
                ));
            }
            if item.laboratory_id == requester_laboratory_id {
                return Err(ApiError::BadRequest(
                    "borrow requests must target another laboratory".into(),
                ));
            }
            if !item.is_cross_lab_borrowable {
                return Err(ApiError::BadRequest(
                    "inventory item is not cross-lab borrowable".into(),
                ));
            }
            if item.tracking_mode == "serialized" && item.status != "available" {
                return Err(ApiError::BadRequest(
                    "serialized inventory item is not available".into(),
                ));
            }
            let available_quantity = item.quantity_on_hand - item.quantity_allocated;
            if payload.requested_quantity > available_quantity {
                return Err(ApiError::BadRequest(
                    "requested_quantity exceeds available quantity".into(),
                ));
            }

            let borrow_request_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO borrow_requests (
                    borrow_request_id,
                    inventory_item_id,
                    requester_user_id,
                    requester_laboratory_id,
                    owner_laboratory_id,
                    requested_quantity,
                    unit_id,
                    expected_borrowed_at,
                    expected_returned_at,
                    purpose
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(borrow_request_id)
            .bind(item.inventory_item_id)
            .bind(actor.user_id)
            .bind(requester_laboratory_id)
            .bind(item.laboratory_id)
            .bind(payload.requested_quantity)
            .bind(item.unit_id)
            .bind(payload.expected_borrowed_at)
            .bind(payload.expected_returned_at)
            .bind(purpose)
            .execute(transaction.as_mut())
            .await
            .map_err(map_database_error)?;

            record_audit(
                &mut transaction,
                &actor,
                Some(item.laboratory_id),
                AuditAction::Create,
                AuditResource::BorrowRequest,
                Some(borrow_request_id),
                json!({
                    "inventory_item_id": item.inventory_item_id,
                    "requested_quantity": payload.requested_quantity
                }),
            )
            .await?;

            let borrow_request =
                fetch_borrow_request_in_transaction(&mut transaction, borrow_request_id).await?;
            let response = HttpResponse::Created().json(borrow_request);
            save_response(transaction, &idempotency_key, *user_id, response).await
        }
    }
}
