use super::model::MyBorrowRequestResponse;
use super::queries::fetch_guest_link_id;
use super::service::{BorrowRequestError, cancel_borrow_request_in_transaction};
use crate::access_control::LaboratoryContext;
use crate::domain::BorrowRequestId;
use actix_web::{HttpResponse, web};
use anyhow::Context;
use sqlx::PgPool;

#[tracing::instrument(
    name = "Cancel borrow request",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, borrow_request_id=%borrow_request_id)
)]
pub async fn cancel_borrow_request(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    borrow_request_id: BorrowRequestId,
) -> Result<HttpResponse, BorrowRequestError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let guest_link_id = fetch_guest_link_id(&mut transaction, actor.user_id, laboratory_id).await?;
    let row = cancel_borrow_request_in_transaction(
        &mut transaction,
        actor,
        laboratory_id,
        *borrow_request_id,
        guest_link_id,
    )
    .await?;

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to cancel a borrow request.")?;

    Ok(HttpResponse::Ok().json(MyBorrowRequestResponse::from(row)))
}
