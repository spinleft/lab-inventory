//! Business flows that chain several statements together, and the rules about
//! who may take part in a borrow.
//!
//! Anything here orchestrates `queries.rs`. Single-statement work belongs in
//! `queries.rs`; HTTP concerns belong in the handler modules. All three handlers
//! answer with the same error type, so it lives here alongside the flows that
//! raise it.
use super::model::{BorrowRequestRow, BorrowRequestStatus, borrow_request_audit_details};
use super::queries::{
    fetch_borrow_actor, fetch_borrow_request_for_update, fetch_borrow_requests_for_guest_link,
    insert_borrow_request, pending_borrow_request_exists, update_borrow_request_cancelled,
};
use crate::access_control::Actor;
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{LaboratoryId, UserId};
use crate::routes::inventory_items::fetch_inventory_item_for_update;
use crate::utils::error_chain_fmt;
use actix_web::ResponseError;
use actix_web::http::StatusCode;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum BorrowRequestError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for BorrowRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for BorrowRequestError {
    fn status_code(&self) -> StatusCode {
        match self {
            BorrowRequestError::ValidationError(_) => StatusCode::BAD_REQUEST,
            BorrowRequestError::Forbidden(_) => StatusCode::FORBIDDEN,
            BorrowRequestError::NotFound(_) => StatusCode::NOT_FOUND,
            BorrowRequestError::ConflictError(_) => StatusCode::CONFLICT,
            BorrowRequestError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Borrowing is for outsiders: you ask another laboratory for something, never
/// your own.
pub(super) fn validate_request_actor(
    actor: &Actor,
    laboratory_id: LaboratoryId,
) -> Result<(), BorrowRequestError> {
    // Federated guests whose home lab matches the target lab
    if actor.is_guest() && actor.laboratory_id == Some(laboratory_id) {
        return Ok(());
    }
    Err(BorrowRequestError::Forbidden(
        "Only guest users can request borrows".into(),
    ))
}

// ---------------------------------------------------------------------------
// flows shared by the session routes and federation
// ---------------------------------------------------------------------------

/// Files a borrow request against an item of `laboratory_id`.
///
/// The transaction is the caller's so that both entry points — a guest signed in
/// here, and a remote user arriving over federation as their shadow guest — run
/// exactly the same statements under exactly the same locks.
///
/// `guest_link_id` is passed in rather than looked up: federation already knows
/// the authoritative link from the signed request that carried the caller, and
/// resolving it again by user id would be a guess when an account has more than
/// one link.
pub(crate) async fn create_borrow_request_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    laboratory_id: LaboratoryId,
    inventory_item_id: Uuid,
    guest_link_id: Option<Uuid>,
    request_note: Option<&str>,
) -> Result<BorrowRequestRow, BorrowRequestError> {
    let item = fetch_inventory_item_for_update(transaction, inventory_item_id)
        .await?
        .ok_or_else(|| BorrowRequestError::NotFound("Inventory item not found".into()))?;
    if item.laboratory_id != *laboratory_id {
        return Err(BorrowRequestError::NotFound(
            "Inventory item not found".into(),
        ));
    }
    validate_request_actor(actor, laboratory_id)?;
    if item.status != "available" {
        return Err(BorrowRequestError::ConflictError(
            "Only available inventory items can be borrowed".into(),
        ));
    }
    // The item is locked for the rest of this transaction and every path that
    // creates or resolves a request takes that same lock, so this check cannot be
    // overtaken by a concurrent one.
    if pending_borrow_request_exists(transaction, item.inventory_item_id).await? {
        return Err(BorrowRequestError::ConflictError(
            "This inventory item already has a pending borrow request".into(),
        ));
    }

    let (requester_username, requester_user_type) =
        fetch_user_snapshot(transaction, actor.user_id).await?;
    let borrow_request_id = Uuid::new_v4();
    insert_borrow_request(
        transaction,
        borrow_request_id,
        laboratory_id,
        item.inventory_item_id,
        actor.user_id,
        &requester_username,
        &requester_user_type,
        guest_link_id,
        request_note,
    )
    .await?;

    let row = fetch_borrow_request_for_update(transaction, *laboratory_id, borrow_request_id)
        .await?
        .ok_or_else(|| BorrowRequestError::NotFound("Borrow request not found".into()))?;
    record_borrow_request_audit(
        transaction,
        actor,
        AuditAction::Create,
        row.borrow_request_id,
        row.inventory_item_id,
        row.status.as_str(),
        row.decision_note.as_deref(),
    )
    .await?;

    Ok(row)
}

/// Retracts a request the caller filed, before anyone has decided on it.
///
/// Nothing about the inventory item changes: filing a request never moved it off
/// `available` in the first place — only an approval does — so a cancellation has
/// nothing to undo. The item is still locked, because that is what serialises
/// this against an approval landing at the same moment.
pub(crate) async fn cancel_borrow_request_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    laboratory_id: LaboratoryId,
    borrow_request_id: Uuid,
    guest_link_id: Option<Uuid>,
) -> Result<BorrowRequestRow, BorrowRequestError> {
    validate_request_actor(actor, laboratory_id)?;
    let row = fetch_borrow_request_for_update(transaction, *laboratory_id, borrow_request_id)
        .await?
        .ok_or_else(|| BorrowRequestError::NotFound("Borrow request not found".into()))?;
    if !cancellable_by(&row, actor, guest_link_id) {
        // Someone else's request is answered as if it did not exist, so this
        // cannot be used to find out which ids are real.
        return Err(BorrowRequestError::NotFound(
            "Borrow request not found".into(),
        ));
    }
    if row.status != BorrowRequestStatus::Pending.as_str() {
        return Err(BorrowRequestError::ConflictError(
            "Borrow request is no longer pending".into(),
        ));
    }

    update_borrow_request_cancelled(transaction, borrow_request_id, laboratory_id).await?;
    let row = fetch_borrow_request_for_update(transaction, *laboratory_id, borrow_request_id)
        .await?
        .ok_or_else(|| BorrowRequestError::NotFound("Borrow request not found".into()))?;
    record_borrow_request_audit(
        transaction,
        actor,
        AuditAction::Update,
        row.borrow_request_id,
        row.inventory_item_id,
        row.status.as_str(),
        row.decision_note.as_deref(),
    )
    .await?;

    Ok(row)
}

/// The requests behind one federation guest link, for a remote caller reading
/// back what they have asked this laboratory for.
pub(crate) async fn list_borrow_requests_for_guest_link(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    guest_link_id: Uuid,
    status: Option<String>,
) -> Result<Vec<BorrowRequestRow>, BorrowRequestError> {
    Ok(fetch_borrow_requests_for_guest_link(pool, laboratory_id, guest_link_id, status).await?)
}

/// Whether this request is the caller's own to retract.
///
/// The guest link is compared only when the caller actually has one. Comparing
/// the two `Option`s directly would make a caller without a link match every
/// request without a link, which is every request a locally registered guest has
/// ever filed.
fn cancellable_by(row: &BorrowRequestRow, actor: &Actor, guest_link_id: Option<Uuid>) -> bool {
    let by_link = match guest_link_id {
        Some(link_id) => row.requester_guest_link_id == Some(link_id),
        None => false,
    };

    by_link || row.requester_user_id == Some(*actor.user_id)
}

/// Deciding on a borrow is the mirror image of asking for one: only the
/// laboratory that owns the item may approve or reject.
pub(super) fn validate_resolver_actor(
    actor: &Actor,
    laboratory_id: LaboratoryId,
) -> Result<(), BorrowRequestError> {
    if (actor.is_lab_admin() || actor.is_regular_user())
        && actor.laboratory_id == Some(laboratory_id)
    {
        Ok(())
    } else {
        Err(BorrowRequestError::Forbidden(
            "Only this laboratory's administrators and users can approve or reject borrow requests"
                .into(),
        ))
    }
}

pub(crate) fn validate_borrow_request_status(
    status: Option<String>,
) -> Result<Option<String>, BorrowRequestError> {
    status
        .map(|status| {
            BorrowRequestStatus::parse(&status)
                .map(|status| status.as_str().to_string())
                .map_err(BorrowRequestError::ValidationError)
        })
        .transpose()
}

/// The name and role a request records for whoever filed or decided it, copied
/// so the request still reads correctly after the account changes.
pub(super) async fn fetch_user_snapshot(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: UserId,
) -> Result<(String, String), BorrowRequestError> {
    let actor = fetch_borrow_actor(transaction, user_id)
        .await?
        .ok_or_else(|| BorrowRequestError::Forbidden("Actor not found in the database".into()))?;

    Ok((actor.username, actor.user_type_name))
}

pub(super) async fn record_borrow_request_audit(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    action: AuditAction,
    borrow_request_id: Uuid,
    inventory_item_id: Uuid,
    status: &str,
    decision_note: Option<&str>,
) -> Result<(), BorrowRequestError> {
    record_audit(
        transaction,
        actor,
        action,
        AuditResource::BorrowRequest,
        Some(borrow_request_id),
        borrow_request_audit_details(borrow_request_id, inventory_item_id, status, decision_note),
    )
    .await
    .map_err(BorrowRequestError::UnexpectedError)
}
