//! Every SQL statement the borrow request routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - functions never return a handler error type, so any handler can reuse them.
//!   Nothing here maps a constraint violation to a message of its own, so plain
//!   [`anyhow::Error`] is enough
use super::model::{BorrowActorRow, BorrowRequestRow};
use crate::domain::{LaboratoryId, UserId};
use anyhow::Context;
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

/// The projection every borrow request read shares. Callers append their own
/// join against `federation_borrow_requests` and their own filters, so the
/// column list lives here rather than being repeated per query.
fn borrow_request_select() -> &'static str {
    r#"
    SELECT
        requests.borrow_request_id,
        requests.local_laboratory_id,
        requests.inventory_item_id,
        requests.requester_user_id,
        requests.requester_username,
        requests.requester_user_type,
        requests.requester_guest_link_id,
        requests.request_note,
        requests.status,
        requests.reviewed_by_user_id,
        requests.reviewed_by_username,
        requests.reviewed_by_user_type,
        requests.reviewed_at,
        requests.decision_note,
        requests.created_at,
        requests.updated_at,
        asset_inventory_items.status AS inventory_status,
        asset_inventory_items.serial_number AS inventory_serial_number,
        asset_inventory_items.batch_number AS inventory_batch_number,
        assets.name AS asset_name,
        assets.model AS asset_model
    FROM asset_inventory_items
    JOIN assets
      ON assets.asset_id = asset_inventory_items.asset_id
    INNER JOIN federation_borrow_requests AS requests
      ON requests.inventory_item_id = asset_inventory_items.inventory_item_id
    "#
}

// ---------------------------------------------------------------------------
// borrow requests
// ---------------------------------------------------------------------------

/// Locks the request and the inventory item behind it together: a decision reads
/// the item's status and writes both rows, so neither may move in between.
pub(super) async fn fetch_borrow_request_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    local_laboratory_id: Uuid,
    borrow_request_id: Uuid,
) -> Result<Option<BorrowRequestRow>, anyhow::Error> {
    let query = format!(
        "{} WHERE requests.local_laboratory_id = $1 AND requests.borrow_request_id = $2 FOR UPDATE OF requests, asset_inventory_items",
        borrow_request_select()
    );
    sqlx::query_as::<_, BorrowRequestRow>(&query)
        .bind(local_laboratory_id)
        .bind(borrow_request_id)
        .fetch_optional(transaction.as_mut())
        .await
        .context("Failed to fetch borrow request for update")
}

pub(super) async fn fetch_borrow_requests(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    status: Option<String>,
) -> Result<Vec<BorrowRequestRow>, anyhow::Error> {
    let mut builder = QueryBuilder::<Postgres>::new(borrow_request_select());
    builder.push(" WHERE requests.local_laboratory_id = ");
    builder.push_bind(*laboratory_id);
    if let Some(status) = status {
        builder.push(" AND requests.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY requests.created_at DESC, requests.borrow_request_id DESC");

    builder
        .build_query_as::<BorrowRequestRow>()
        .fetch_all(pool)
        .await
        .context("Failed to fetch borrow requests")
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Saving new borrow request in the database",
    skip(transaction, requester_username, requester_user_type, request_note),
    fields(laboratory_id=%laboratory_id)
)]
pub(super) async fn insert_borrow_request(
    transaction: &mut Transaction<'_, Postgres>,
    borrow_request_id: Uuid,
    laboratory_id: LaboratoryId,
    inventory_item_id: Uuid,
    requester_user_id: UserId,
    requester_username: &str,
    requester_user_type: &str,
    guest_link_id: Uuid,
    request_note: Option<&str>,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        INSERT INTO federation_borrow_requests (
            borrow_request_id,
            local_laboratory_id,
            inventory_item_id,
            requester_user_id,
            requester_username,
            requester_user_type,
            requester_guest_link_id,
            request_note,
            status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending')
        "#,
        borrow_request_id,
        *laboratory_id,
        inventory_item_id,
        *requester_user_id,
        requester_username,
        requester_user_type,
        guest_link_id,
        request_note,
    )
    .execute(transaction.as_mut())
    .await
    .context("Failed to store a new borrow request")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Recording borrow request decision in the database",
    skip(transaction, reviewer_username, reviewer_user_type, decision_note),
    fields(borrow_request_id=%borrow_request_id)
)]
pub(super) async fn update_borrow_request_decision(
    transaction: &mut Transaction<'_, Postgres>,
    borrow_request_id: Uuid,
    laboratory_id: LaboratoryId,
    status: &str,
    reviewed_by_user_id: UserId,
    reviewer_username: &str,
    reviewer_user_type: &str,
    decision_note: Option<&str>,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
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
        borrow_request_id,
        *laboratory_id,
        status,
        *reviewed_by_user_id,
        reviewer_username,
        reviewer_user_type,
        decision_note,
    )
    .execute(transaction.as_mut())
    .await
    .context("Failed to record the borrow request decision")?;

    Ok(())
}

pub(super) async fn mark_inventory_item_borrowed(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        UPDATE asset_inventory_items
        SET status = 'borrowed',
            updated_at = now()
        WHERE inventory_item_id = $1
        "#,
        inventory_item_id,
    )
    .execute(transaction.as_mut())
    .await
    .context("Failed to mark the inventory item as borrowed")?;

    Ok(())
}

// ---------------------------------------------------------------------------
// requesters
// ---------------------------------------------------------------------------

pub(super) async fn fetch_guest_link_id(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: UserId,
    laboratory_id: LaboratoryId,
) -> Result<Option<Uuid>, anyhow::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT link_id
        FROM federation_guest_links
        WHERE local_guest_user_id = $1
          AND local_laboratory_id = $2
        "#,
        *user_id,
        *laboratory_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch federation guest link")
}

pub(super) async fn fetch_borrow_actor(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: UserId,
) -> Result<Option<BorrowActorRow>, anyhow::Error> {
    sqlx::query_as!(
        BorrowActorRow,
        r#"
        SELECT users.username, user_types.name AS user_type_name
        FROM users
        JOIN user_types USING (user_type_id)
        WHERE users.user_id = $1
        "#,
        *user_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch borrow request actor")
}
