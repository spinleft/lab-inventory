//! Business flows that chain several statements together, and the rules about
//! who may take part in a borrow.
//!
//! Anything here orchestrates `queries.rs`. Single-statement work belongs in
//! `queries.rs`; HTTP concerns belong in the handler modules. All three handlers
//! answer with the same error type, so it lives here alongside the flows that
//! raise it.
use super::model::{BorrowRequestStatus, borrow_request_audit_details};
use super::queries::{fetch_borrow_actor, fetch_guest_link_id, insert_local_guest_link};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{LaboratoryId, UserId};
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

pub(super) async fn actor_for_user(
    pool: &PgPool,
    actor_user_id: UserId,
) -> Result<Actor, BorrowRequestError> {
    get_actor(pool, actor_user_id)
        .await
        .map_err(BorrowRequestError::UnexpectedError)?
        .ok_or_else(|| BorrowRequestError::Forbidden("Actor not found in the database".into()))
}

pub(super) fn validate_inventory_item_read_permission(
    actor: &Actor,
    laboratory_id: Uuid,
) -> Result<LaboratoryId, BorrowRequestError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id)
        .map_err(|e| BorrowRequestError::UnexpectedError(anyhow::anyhow!(e)))?;
    if actor.can_query_laboratory_resource(&laboratory_id) {
        Ok(laboratory_id)
    } else {
        Err(BorrowRequestError::Forbidden(
            "You do not have permission to view inventory items for this laboratory".into(),
        ))
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
    // Same-server cross-laboratory admins and users can also request borrows;
    // they will be auto-registered as local guest links.
    if (actor.is_lab_admin() || actor.is_regular_user())
        && actor.laboratory_id.is_some()
        && actor.laboratory_id != Some(laboratory_id)
    {
        return Ok(());
    }
    Err(BorrowRequestError::Forbidden(
        "Only guest users or cross-laboratory users can request borrows".into(),
    ))
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

pub(super) fn validate_borrow_request_status(
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

/// The guest link a borrow request is filed under.
///
/// A federated guest already has one. A user of another laboratory on this same
/// server does not, so one is created on the spot — that is what lets them
/// borrow on the same footing as a federated guest.
pub(super) async fn resolve_guest_link_id(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: UserId,
    laboratory_id: LaboratoryId,
) -> Result<Uuid, BorrowRequestError> {
    if let Some(link_id) = fetch_guest_link_id(transaction, user_id, laboratory_id).await? {
        return Ok(link_id);
    }

    let actor = fetch_borrow_actor(transaction, user_id)
        .await?
        .ok_or_else(|| BorrowRequestError::Forbidden("Actor not found in the database".into()))?;
    let is_cross_lab = matches!(actor.user_type_name.as_str(), "lab_admin" | "user")
        && actor.laboratory_id.is_some()
        && actor.laboratory_id != Some(*laboratory_id);
    if !is_cross_lab {
        return Err(BorrowRequestError::Forbidden(
            "Only federated guest users or cross-laboratory users can create borrow requests"
                .into(),
        ));
    }

    let home_laboratory_id = actor
        .laboratory_id
        .ok_or_else(|| BorrowRequestError::Forbidden("Actor has no home laboratory".into()))?;
    let link_id = Uuid::new_v4();
    insert_local_guest_link(
        transaction,
        link_id,
        laboratory_id,
        home_laboratory_id,
        user_id,
        &actor.username,
        &actor.user_type_name,
    )
    .await?;

    Ok(link_id)
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
