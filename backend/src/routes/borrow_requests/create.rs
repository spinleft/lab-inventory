use super::model::BorrowRequestResponse;
use super::queries::fetch_guest_link_id;
use super::service::{BorrowRequestError, create_borrow_request_in_transaction};
use crate::access_control::LaboratoryContext;
use crate::domain::InventoryItemId;
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBorrowRequestJsonData {
    request_note: Option<String>,
}

#[tracing::instrument(
    name = "Create borrow request",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn create_borrow_request(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    inventory_item_id: InventoryItemId,
    payload: web::Json<CreateBorrowRequestJsonData>,
) -> Result<HttpResponse, BorrowRequestError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    let request_note = payload
        .request_note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    // A guest who reached this laboratory through federation has a link tying the
    // request back to their remote identity. One who registered here directly has
    // none, and borrows without one.
    let guest_link_id = fetch_guest_link_id(&mut transaction, actor.user_id, laboratory_id).await?;
    let row = create_borrow_request_in_transaction(
        &mut transaction,
        actor,
        laboratory_id,
        *inventory_item_id,
        guest_link_id,
        request_note,
    )
    .await?;

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to create a borrow request.")?;

    Ok(HttpResponse::Created().json(BorrowRequestResponse::from(row)))
}
