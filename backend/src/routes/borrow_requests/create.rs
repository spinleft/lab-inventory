use super::model::{
    BorrowRequestError, BorrowRequestResponse, actor_for_user, fetch_borrow_request_for_update,
    fetch_guest_link_id, fetch_user_snapshot, record_borrow_request_audit,
    validate_inventory_item_read_permission, validate_request_actor,
};
use crate::audit::AuditAction;
use crate::domain::UserId;
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
    fields(actor_user_id=%actor_user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn create_borrow_request(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
    payload: web::Json<CreateBorrowRequestBody>,
) -> Result<HttpResponse, BorrowRequestError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let item = fetch_inventory_item_for_update(&mut transaction, inventory_item_id.into_inner())
        .await?
        .ok_or_else(|| BorrowRequestError::NotFound("Inventory item not found".into()))?;
    let laboratory_id = validate_inventory_item_read_permission(&actor, item.laboratory_id)?;
    validate_request_actor(&actor, laboratory_id)?;
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
    let guest_link_id = fetch_guest_link_id(&mut transaction, actor.user_id, laboratory_id).await?;
    let (requester_username, requester_user_type) =
        fetch_user_snapshot(&mut transaction, actor.user_id).await?;
    let borrow_request_id = Uuid::new_v4();

    sqlx::query(
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
    )
    .bind(borrow_request_id)
    .bind(*laboratory_id)
    .bind(item.inventory_item_id)
    .bind(*actor.user_id)
    .bind(requester_username)
    .bind(requester_user_type)
    .bind(guest_link_id)
    .bind(request_note)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| BorrowRequestError::UnexpectedError(e.into()))?;

    let row = fetch_borrow_request_for_update(&mut transaction, *laboratory_id, borrow_request_id)
        .await?
        .ok_or_else(|| BorrowRequestError::NotFound("Borrow request not found".into()))?;
    record_borrow_request_audit(
        &mut transaction,
        &actor,
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
