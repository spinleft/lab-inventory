use super::model::{
    InventoryItemError, actor_for_user, delete_inventory_item_attachments,
    delete_inventory_item_from_database, delete_inventory_item_rollback_details,
    fetch_inventory_item_for_update, record_inventory_item_audit, record_inventory_transaction,
    validate_write_permission,
};
use crate::attachment_storage::AttachmentStorage;
use crate::audit::AuditAction;
use crate::domain::UserId;
use crate::routes::attachments::delete_storage_objects;
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(
    name = "Delete an inventory item",
    skip(pool, storage),
    fields(actor_user_id=%actor_user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn delete_inventory_item(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<AttachmentStorage>,
    inventory_item_id: web::Path<Uuid>,
) -> Result<HttpResponse, InventoryItemError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let item = fetch_inventory_item_for_update(&mut transaction, inventory_item_id.into_inner())
        .await?
        .ok_or_else(|| InventoryItemError::NotFound("Inventory item not found".into()))?;
    validate_write_permission(&actor, item.laboratory_id)?;
    if item.quantity_allocated > 0.0 {
        return Err(InventoryItemError::ConflictError(
            "Cannot delete inventory items with allocated quantity".into(),
        ));
    }

    let deleted_attachments =
        delete_inventory_item_attachments(&mut transaction, item.inventory_item_id).await?;
    let attachment_ids = deleted_attachments
        .iter()
        .map(|row| row.attachment_id)
        .collect::<Vec<_>>();
    record_inventory_transaction(
        &mut transaction,
        &actor,
        &item,
        "delete",
        -item.quantity_on_hand,
        -item.quantity_allocated,
        item.location_id,
        None,
        json!({
            "operation": "delete",
            "inventory_item": item,
        }),
    )
    .await?;
    delete_inventory_item_from_database(&mut transaction, item.inventory_item_id).await?;
    record_inventory_item_audit(
        &mut transaction,
        &actor,
        AuditAction::Delete,
        item.inventory_item_id,
        delete_inventory_item_rollback_details(&item, &attachment_ids),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to delete an inventory item.")?;
    delete_storage_objects(&storage, &deleted_attachments).await?;

    Ok(HttpResponse::NoContent().finish())
}
