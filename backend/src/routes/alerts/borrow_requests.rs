use super::model::BorrowRequestAlert;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

#[tracing::instrument(name = "List borrow request alerts", skip(pool), fields(user_id=%user_id))]
pub async fn list_borrow_request_alerts(
    user_id: UserId,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let laboratory_id = actor.laboratory_id;
    let alerts = sqlx::query_as::<_, BorrowRequestAlert>(
        r#"
        SELECT
            borrow_requests.borrow_request_id,
            CASE
                WHEN borrow_requests.status = 'pending' THEN 'pending_approval'
                WHEN borrow_requests.status = 'approved' THEN 'pending_borrow_out'
                WHEN borrow_requests.status = 'borrowed'
                    AND borrow_requests.expected_returned_at IS NOT NULL
                    AND borrow_requests.expected_returned_at < now()
                    THEN 'overdue'
                ELSE 'pending_return'
            END AS alert_kind,
            borrow_requests.inventory_item_id,
            asset_inventory_items.asset_id,
            assets.name AS asset_name,
            assets.model AS asset_model,
            borrow_requests.requester_user_id,
            requester.username AS requester_username,
            borrow_requests.requester_laboratory_id,
            requester_laboratory.name AS requester_laboratory_name,
            borrow_requests.owner_laboratory_id,
            owner_laboratory.name AS owner_laboratory_name,
            borrow_requests.requested_quantity,
            borrow_requests.unit_id,
            units.code AS unit_code,
            borrow_requests.expected_borrowed_at,
            borrow_requests.expected_returned_at,
            borrow_requests.purpose,
            borrow_requests.status,
            borrow_requests.created_at,
            borrow_requests.updated_at
        FROM borrow_requests
        INNER JOIN asset_inventory_items USING (inventory_item_id)
        INNER JOIN assets USING (asset_id)
        INNER JOIN users AS requester ON requester.user_id = borrow_requests.requester_user_id
        INNER JOIN laboratories AS requester_laboratory ON requester_laboratory.laboratory_id = borrow_requests.requester_laboratory_id
        INNER JOIN laboratories AS owner_laboratory ON owner_laboratory.laboratory_id = borrow_requests.owner_laboratory_id
        INNER JOIN units ON units.unit_id = borrow_requests.unit_id
        WHERE borrow_requests.status IN ('pending', 'approved', 'borrowed')
          AND (
              $1::bool
              OR borrow_requests.requester_laboratory_id = $2
              OR borrow_requests.owner_laboratory_id = $2
          )
        ORDER BY
            CASE
                WHEN borrow_requests.status = 'borrowed'
                    AND borrow_requests.expected_returned_at IS NOT NULL
                    AND borrow_requests.expected_returned_at < now()
                    THEN 0
                WHEN borrow_requests.status = 'pending' THEN 1
                WHEN borrow_requests.status = 'approved' THEN 2
                ELSE 3
            END,
            borrow_requests.created_at DESC
        "#,
    )
    .bind(actor.is_owner())
    .bind(laboratory_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(alerts))
}
