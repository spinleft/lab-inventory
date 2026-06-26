use super::model::{
    InventoryItemError, actor_for_user, delete_inventory_item_attachments,
    delete_inventory_item_from_database, delete_inventory_item_rollback_details,
    fetch_inventory_items_for_update, record_inventory_item_audit, record_inventory_transaction,
    validate_requested_ids, validate_write_permission,
};
use crate::attachment_storage::AttachmentStorage;
use crate::audit::AuditAction;
use crate::domain::UserId;
use crate::routes::attachments::delete_storage_objects;
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    inventory_item_ids: Vec<Uuid>,
}

#[tracing::instrument(
    name = "Batch delete inventory items",
    skip(pool, storage, payload),
    fields(actor_user_id=%actor_user_id)
)]
pub async fn batch_delete_inventory_items(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    storage: web::Data<AttachmentStorage>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, InventoryItemError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let payload = payload.into_inner();
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let items =
        fetch_inventory_items_for_update(&mut transaction, &payload.inventory_item_ids).await?;
    validate_requested_ids(&payload.inventory_item_ids, &items)?;
    for item in &items {
        validate_write_permission(&actor, item.laboratory_id)?;
        if item.quantity_allocated > 0.0 {
            return Err(InventoryItemError::ConflictError(
                "Cannot delete inventory items with allocated quantity".into(),
            ));
        }
    }

    let mut deleted_attachments = Vec::new();
    for item in items {
        let item_deleted_attachments =
            delete_inventory_item_attachments(&mut transaction, item.inventory_item_id).await?;
        let attachment_ids = item_deleted_attachments
            .iter()
            .map(|row| row.attachment_id)
            .collect::<Vec<_>>();
        deleted_attachments.extend(item_deleted_attachments);
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
                "operation": "batch_delete",
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
    }

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to batch delete inventory items.")?;
    delete_storage_objects(&storage, &deleted_attachments).await?;
    Ok(HttpResponse::NoContent().finish())
}
