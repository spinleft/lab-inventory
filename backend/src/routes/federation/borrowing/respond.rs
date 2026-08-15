//! Turns a federation borrow path into an HTTP response.
//!
//! The inbound endpoint takes an arbitrary tail path rather than a set of typed
//! routes, so the routing Actix would normally do happens here, as it does for
//! reads in `public_data::respond`.
use super::model::{FederationBorrowTarget, FederationCreateBorrowRequestBody};
use crate::access_control::{Actor, get_actor};
use crate::domain::{LaboratoryId, UserId};
use crate::routes::borrow_requests::{
    BorrowRequestError, MyBorrowRequestResponse, cancel_borrow_request_in_transaction,
    create_borrow_request_in_transaction, list_borrow_requests_for_guest_link,
    validate_borrow_request_status,
};
use crate::routes::federation::model::GuestLinkIdentity;
use actix_web::HttpResponse;
use actix_web::http::Method;
use anyhow::Context;
use sqlx::PgPool;
use url::form_urlencoded;
use uuid::Uuid;

/// Resolves a tail path against the borrow surface.
///
/// `None` means "not a borrow route" rather than "no such route", so a GET can
/// fall through to the public read parser and answer exactly as it does today.
/// A malformed id is also `None`: it is not a borrow route either, and letting
/// it fall through keeps one wording for every unknown path.
pub(crate) fn parse_borrow_target(method: &Method, tail: &str) -> Option<FederationBorrowTarget> {
    let mut parts = tail
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty());
    let first = parts.next();
    let second = parts.next();
    let third = parts.next();
    if parts.next().is_some() {
        return None;
    }
    match (method, first, second, third) {
        (&Method::GET, Some("borrow-requests"), None, None) => {
            Some(FederationBorrowTarget::ListMine)
        }
        (&Method::POST, Some("inventory-items"), Some(item_id), Some("borrow-requests")) => {
            Uuid::parse_str(item_id).ok().map(FederationBorrowTarget::Create)
        }
        (&Method::POST, Some("borrow-requests"), Some(request_id), Some("cancel")) => {
            Uuid::parse_str(request_id)
                .ok()
                .map(FederationBorrowTarget::Cancel)
        }
        _ => None,
    }
}

/// Serves a borrow operation on behalf of the remote user behind `caller`.
///
/// The guest link is resolved into a real [`Actor`] and handed to the same flows
/// the session routes use, so a federated request is authorized by the same
/// rules as a local one rather than by a parallel set that could drift from it.
pub(crate) async fn respond_federation_borrow(
    pool: &PgPool,
    laboratory_id: Uuid,
    caller: &GuestLinkIdentity,
    target: FederationBorrowTarget,
    query_string: &str,
    body: &[u8],
) -> Result<HttpResponse, BorrowRequestError> {
    let laboratory_id: LaboratoryId = laboratory_id.into();
    let actor = federated_actor(pool, caller).await?;

    match target {
        FederationBorrowTarget::ListMine => {
            let status = validate_borrow_request_status(status_filter(query_string))?;
            let requests =
                list_borrow_requests_for_guest_link(pool, laboratory_id, caller.link_id, status)
                    .await?;

            Ok(HttpResponse::Ok().json(
                requests
                    .into_iter()
                    .map(MyBorrowRequestResponse::from)
                    .collect::<Vec<_>>(),
            ))
        }
        FederationBorrowTarget::Create(inventory_item_id) => {
            let payload: FederationCreateBorrowRequestBody = serde_json::from_slice(body)
                .map_err(|error| BorrowRequestError::ValidationError(error.to_string()))?;
            let request_note = payload
                .request_note
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let mut transaction = pool
                .begin()
                .await
                .context("Failed to acquire a Postgres connection from the pool")?;
            let row = create_borrow_request_in_transaction(
                &mut transaction,
                &actor,
                laboratory_id,
                inventory_item_id,
                Some(caller.link_id),
                request_note,
            )
            .await?;
            transaction.commit().await.context(
                "Failed to commit SQL transaction to create a federated borrow request.",
            )?;

            Ok(HttpResponse::Created().json(MyBorrowRequestResponse::from(row)))
        }
        FederationBorrowTarget::Cancel(borrow_request_id) => {
            let mut transaction = pool
                .begin()
                .await
                .context("Failed to acquire a Postgres connection from the pool")?;
            let row = cancel_borrow_request_in_transaction(
                &mut transaction,
                &actor,
                laboratory_id,
                borrow_request_id,
                Some(caller.link_id),
            )
            .await?;
            transaction.commit().await.context(
                "Failed to commit SQL transaction to cancel a federated borrow request.",
            )?;

            Ok(HttpResponse::Ok().json(MyBorrowRequestResponse::from(row)))
        }
    }
}

/// The shadow guest as the local authorization rules see it.
///
/// It is read back from the database rather than assembled here so that a
/// federated caller and a signed-in one are the same kind of thing: whatever the
/// account is, that is what the borrow rules are applied to.
async fn federated_actor(
    pool: &PgPool,
    caller: &GuestLinkIdentity,
) -> Result<Actor, BorrowRequestError> {
    let user_id: UserId = caller.local_guest_user_id.into();
    get_actor(pool, user_id)
        .await?
        .ok_or_else(|| {
            BorrowRequestError::UnexpectedError(anyhow::anyhow!(
                "Federation guest link points at a user that does not exist"
            ))
        })
}

/// The only filter this surface takes. It arrives unparsed because the tail
/// route bypasses Actix's typed extractors.
fn status_filter(query_string: &str) -> Option<String> {
    form_urlencoded::parse(query_string.as_bytes())
        .find(|(key, _)| key == "status")
        .map(|(_, value)| value.into_owned())
}
