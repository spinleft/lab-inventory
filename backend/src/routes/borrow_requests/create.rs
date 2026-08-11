use super::model::BorrowRequestResponse;
use super::queries::{fetch_borrow_request_for_update, insert_borrow_request};
use super::service::{
    BorrowRequestError, fetch_user_snapshot, record_borrow_request_audit, resolve_guest_link_id,
    validate_request_actor,
};
use crate::access_control::{InventoryItemPathId, LaboratoryContext};
use crate::audit::AuditAction;
use crate::routes::inventory_items::fetch_inventory_item_for_update;
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBorrowRequestBody {
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
    inventory_item_id: InventoryItemPathId,
    payload: web::Json<CreateBorrowRequestBody>,
) -> Result<HttpResponse, BorrowRequestError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let item = fetch_inventory_item_for_update(&mut transaction, inventory_item_id.into_inner())
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

    let request_note = payload
        .request_note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let guest_link_id =
        resolve_guest_link_id(&mut transaction, actor.user_id, laboratory_id).await?;
    let (requester_username, requester_user_type) =
        fetch_user_snapshot(&mut transaction, actor.user_id).await?;
    let borrow_request_id = Uuid::new_v4();

    insert_borrow_request(
        &mut transaction,
        borrow_request_id,
        laboratory_id,
        item.inventory_item_id,
        actor.user_id,
        &requester_username,
        &requester_user_type,
        guest_link_id,
        request_note.as_deref(),
    )
    .await?;

    let row = fetch_borrow_request_for_update(&mut transaction, *laboratory_id, borrow_request_id)
        .await?
        .ok_or_else(|| BorrowRequestError::NotFound("Borrow request not found".into()))?;
    record_borrow_request_audit(
        &mut transaction,
        actor,
        AuditAction::Create,
        row.borrow_request_id,
        row.inventory_item_id,
        row.status.as_str(),
        row.decision_note.as_deref(),
    )
    .await?;

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to create a borrow request.")?;

    Ok(HttpResponse::Created().json(BorrowRequestResponse::from(row)))
}
