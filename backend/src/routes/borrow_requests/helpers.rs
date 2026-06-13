use super::model::{BorrowRequest, BorrowRequestForUpdate, InventoryItemForBorrow};
use crate::audit::AuditAction;
use crate::authentication::{ADMIN, Actor, USER};
use crate::utils::ApiError;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(super) fn borrow_request_select() -> &'static str {
    r#"
    SELECT
        borrow_requests.borrow_request_id,
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
        borrow_requests.reviewed_by_user_id,
        borrow_requests.reviewed_at,
        borrow_requests.review_comment,
        borrow_requests.borrowed_by_user_id,
        borrow_requests.borrowed_at,
        borrow_requests.returned_by_user_id,
        borrow_requests.returned_at,
        borrow_requests.cancelled_by_user_id,
        borrow_requests.cancelled_at,
        borrow_requests.created_at,
        borrow_requests.updated_at
    FROM borrow_requests
    INNER JOIN asset_inventory_items USING (inventory_item_id)
    INNER JOIN assets USING (asset_id)
    INNER JOIN users AS requester ON requester.user_id = borrow_requests.requester_user_id
    INNER JOIN laboratories AS requester_laboratory ON requester_laboratory.laboratory_id = borrow_requests.requester_laboratory_id
    INNER JOIN laboratories AS owner_laboratory ON owner_laboratory.laboratory_id = borrow_requests.owner_laboratory_id
    INNER JOIN units ON units.unit_id = borrow_requests.unit_id
    "#
}

pub(super) async fn fetch_borrow_request(
    pool: &PgPool,
    borrow_request_id: Uuid,
) -> Result<BorrowRequest, ApiError> {
    let query = format!(
        "{} WHERE borrow_requests.borrow_request_id = $1",
        borrow_request_select()
    );
    sqlx::query_as::<_, BorrowRequest>(&query)
        .bind(borrow_request_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound)
}

pub(super) async fn fetch_borrow_request_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    borrow_request_id: Uuid,
) -> Result<BorrowRequest, ApiError> {
    let query = format!(
        "{} WHERE borrow_requests.borrow_request_id = $1",
        borrow_request_select()
    );
    sqlx::query_as::<_, BorrowRequest>(&query)
        .bind(borrow_request_id)
        .fetch_optional(transaction.as_mut())
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?
        .ok_or(ApiError::NotFound)
}

pub(super) async fn fetch_borrow_request_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    borrow_request_id: Uuid,
) -> Result<BorrowRequestForUpdate, ApiError> {
    sqlx::query_as::<_, BorrowRequestForUpdate>(
        r#"
        SELECT
            borrow_request_id,
            inventory_item_id,
            requester_laboratory_id,
            owner_laboratory_id,
            requested_quantity,
            status
        FROM borrow_requests
        WHERE borrow_request_id = $1
        FOR UPDATE
        "#,
    )
    .bind(borrow_request_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::NotFound)
}

pub(super) async fn fetch_inventory_item_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
) -> Result<InventoryItemForBorrow, ApiError> {
    sqlx::query_as::<_, InventoryItemForBorrow>(
        r#"
        SELECT
            asset_inventory_items.inventory_item_id,
            asset_inventory_items.laboratory_id,
            asset_inventory_items.tracking_mode,
            asset_inventory_items.quantity_on_hand,
            asset_inventory_items.quantity_allocated,
            asset_inventory_items.unit_id,
            units.allow_decimal AS unit_allow_decimal,
            asset_inventory_items.is_cross_lab_borrowable,
            asset_inventory_items.status,
            asset_inventory_items.location_id
        FROM asset_inventory_items
        INNER JOIN units ON units.unit_id = asset_inventory_items.unit_id
        WHERE asset_inventory_items.inventory_item_id = $1
        FOR UPDATE
        "#,
    )
    .bind(inventory_item_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::NotFound)
}

pub(super) fn ensure_can_create(actor: &Actor) -> Result<Uuid, ApiError> {
    if matches!(actor.user_type_name.as_str(), ADMIN | USER) {
        actor.laboratory_id.ok_or(ApiError::Forbidden)
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) fn ensure_can_view(actor: &Actor, request: &BorrowRequest) -> Result<(), ApiError> {
    if actor.is_owner()
        || actor.laboratory_id == Some(request.requester_laboratory_id)
        || actor.laboratory_id == Some(request.owner_laboratory_id)
    {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) fn ensure_can_owner_operate(
    actor: &Actor,
    owner_laboratory_id: Uuid,
) -> Result<(), ApiError> {
    if actor.is_owner()
        || (matches!(actor.user_type_name.as_str(), ADMIN | USER)
            && actor.laboratory_id == Some(owner_laboratory_id))
    {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) fn ensure_can_cancel(
    actor: &Actor,
    request: &BorrowRequestForUpdate,
) -> Result<(), ApiError> {
    if actor.is_owner()
        || (matches!(actor.user_type_name.as_str(), ADMIN | USER)
            && (actor.laboratory_id == Some(request.requester_laboratory_id)
                || actor.laboratory_id == Some(request.owner_laboratory_id)))
    {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub(super) fn validate_positive_quantity(
    quantity: f64,
    allow_decimal: bool,
) -> Result<(), ApiError> {
    if !quantity.is_finite() || quantity <= 0.0 {
        return Err(ApiError::BadRequest(
            "requested_quantity must be positive".into(),
        ));
    }
    if !allow_decimal && quantity.fract().abs() > f64::EPSILON {
        return Err(ApiError::BadRequest(
            "requested_quantity must be an integer".into(),
        ));
    }
    Ok(())
}

pub(super) fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error
        && let Some("23514") = database_error.code().as_deref()
    {
        return ApiError::BadRequest("Invalid borrow request data".into());
    }
    ApiError::UnexpectedError(error.into())
}

pub(super) async fn update_inventory_after_approval(
    transaction: &mut Transaction<'_, Postgres>,
    item: &InventoryItemForBorrow,
    requested_quantity: f64,
) -> Result<(), ApiError> {
    let status = if item.tracking_mode == "serialized" {
        "reserved"
    } else {
        item.status.as_str()
    };
    sqlx::query(
        r#"
        UPDATE asset_inventory_items
        SET quantity_allocated = quantity_allocated + $2,
            status = $3,
            updated_at = now()
        WHERE inventory_item_id = $1
        "#,
    )
    .bind(item.inventory_item_id)
    .bind(requested_quantity)
    .bind(status)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    Ok(())
}

pub(super) async fn update_inventory_after_borrowed(
    transaction: &mut Transaction<'_, Postgres>,
    item: &InventoryItemForBorrow,
) -> Result<(), ApiError> {
    if item.tracking_mode == "serialized" {
        sqlx::query(
            r#"
            UPDATE asset_inventory_items
            SET status = 'borrowed', updated_at = now()
            WHERE inventory_item_id = $1
            "#,
        )
        .bind(item.inventory_item_id)
        .execute(transaction.as_mut())
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    }
    Ok(())
}

pub(super) async fn update_inventory_after_returned(
    transaction: &mut Transaction<'_, Postgres>,
    item: &InventoryItemForBorrow,
    requested_quantity: f64,
) -> Result<(), ApiError> {
    let status = if item.tracking_mode == "serialized" {
        "available"
    } else {
        item.status.as_str()
    };
    sqlx::query(
        r#"
        UPDATE asset_inventory_items
        SET quantity_allocated = quantity_allocated - $2,
            status = $3,
            updated_at = now()
        WHERE inventory_item_id = $1
        "#,
    )
    .bind(item.inventory_item_id)
    .bind(requested_quantity)
    .bind(status)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    Ok(())
}

pub(super) struct InventoryTransactionData {
    pub inventory_item_id: Uuid,
    pub laboratory_id: Uuid,
    pub actor_user_id: Uuid,
    pub actor_laboratory_id: Option<Uuid>,
    pub action: AuditAction,
    pub quantity_delta: f64,
    pub allocated_delta: f64,
    pub from_location_id: Option<Uuid>,
    pub to_location_id: Option<Uuid>,
    pub borrow_request_id: Uuid,
    pub details: Value,
}

pub(super) async fn record_inventory_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    data: InventoryTransactionData,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO inventory_transactions (
            transaction_id,
            inventory_item_id,
            laboratory_id,
            actor_user_id,
            actor_laboratory_id,
            action,
            quantity_delta,
            allocated_delta,
            from_location_id,
            to_location_id,
            related_resource_type,
            related_resource_id,
            details
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'borrow_request', $11, $12)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(data.inventory_item_id)
    .bind(data.laboratory_id)
    .bind(data.actor_user_id)
    .bind(data.actor_laboratory_id)
    .bind(data.action.as_str())
    .bind(data.quantity_delta)
    .bind(data.allocated_delta)
    .bind(data.from_location_id)
    .bind(data.to_location_id)
    .bind(data.borrow_request_id)
    .bind(data.details)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;
    Ok(())
}
