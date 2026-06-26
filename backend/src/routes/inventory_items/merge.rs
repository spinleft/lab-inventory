use super::model::{
    InventoryItemError, InventoryItemResponse, actor_for_user, add_quantities_to_item,
    delete_inventory_item_from_database, fetch_inventory_items_for_update,
    merge_inventory_items_rollback_details, move_inventory_item_attachments,
    record_inventory_item_audit, record_inventory_transaction, validate_quantity_item,
    validate_requested_ids, validate_write_permission,
};
use crate::audit::AuditAction;
use crate::domain::UserId;
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    target_inventory_item_id: Uuid,
    source_inventory_item_ids: Vec<Uuid>,
}

#[tracing::instrument(
    name = "Merge inventory items",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id)
)]
pub async fn merge_inventory_items(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, InventoryItemError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let payload = payload.into_inner();
    if payload.source_inventory_item_ids.is_empty() {
        return Err(InventoryItemError::ValidationError(
            "source_inventory_item_ids cannot be empty".into(),
        ));
    }
    if payload
        .source_inventory_item_ids
        .contains(&payload.target_inventory_item_id)
    {
        return Err(InventoryItemError::ValidationError(
            "target_inventory_item_id cannot be included in source_inventory_item_ids".into(),
        ));
    }

    let mut all_ids = Vec::with_capacity(payload.source_inventory_item_ids.len() + 1);
    all_ids.push(payload.target_inventory_item_id);
    all_ids.extend(payload.source_inventory_item_ids.iter().copied());

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let rows = fetch_inventory_items_for_update(&mut transaction, &all_ids).await?;
    validate_requested_ids(&all_ids, &rows)?;

    let target_before = rows
        .iter()
        .find(|row| row.inventory_item_id == payload.target_inventory_item_id)
        .cloned()
        .ok_or_else(|| InventoryItemError::NotFound("Target inventory item not found".into()))?;
    validate_write_permission(&actor, target_before.laboratory_id)?;
    validate_quantity_item(&target_before)?;

    let mut sources = Vec::with_capacity(payload.source_inventory_item_ids.len());
    for source_id in &payload.source_inventory_item_ids {
        let source = rows
            .iter()
            .find(|row| row.inventory_item_id == *source_id)
            .cloned()
            .ok_or_else(|| {
                InventoryItemError::NotFound("Source inventory item not found".into())
            })?;
        validate_write_permission(&actor, source.laboratory_id)?;
        validate_quantity_item(&source)?;
        validate_merge_compatible(&target_before, &source)?;
        sources.push(source);
    }

    let quantity_delta: f64 = sources.iter().map(|source| source.quantity_on_hand).sum();
    let allocated_delta: f64 = sources.iter().map(|source| source.quantity_allocated).sum();
    let source_ids = sources
        .iter()
        .map(|source| source.inventory_item_id)
        .collect::<Vec<_>>();

    let moved_attachment_ids = move_inventory_item_attachments(
        &mut transaction,
        &source_ids,
        target_before.inventory_item_id,
    )
    .await?;
    let target_after = add_quantities_to_item(
        &mut transaction,
        target_before.inventory_item_id,
        quantity_delta,
        allocated_delta,
    )
    .await?;

    for source in &sources {
        record_inventory_transaction(
            &mut transaction,
            &actor,
            source,
            "delete",
            -source.quantity_on_hand,
            -source.quantity_allocated,
            source.location_id,
            target_after.location_id,
            json!({
                "operation": "merge",
                "role": "source",
                "target_inventory_item_id": target_after.inventory_item_id,
                "source": source,
            }),
        )
        .await?;
        delete_inventory_item_from_database(&mut transaction, source.inventory_item_id).await?;
    }
    record_inventory_transaction(
        &mut transaction,
        &actor,
        &target_after,
        "adjust",
        quantity_delta,
        allocated_delta,
        target_before.location_id,
        target_after.location_id,
        json!({
            "operation": "merge",
            "role": "target",
            "target_before": target_before,
            "target_after": target_after,
            "source_inventory_item_ids": source_ids,
        }),
    )
    .await?;
    record_inventory_item_audit(
        &mut transaction,
        &actor,
        AuditAction::Adjust,
        target_after.inventory_item_id,
        merge_inventory_items_rollback_details(&target_before, &sources, &moved_attachment_ids),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to merge inventory items.")?;

    Ok(HttpResponse::Ok().json(InventoryItemResponse::from(target_after)))
}

fn validate_merge_compatible(
    target: &super::model::InventoryItemRow,
    source: &super::model::InventoryItemRow,
) -> Result<(), InventoryItemError> {
    if target.laboratory_id == source.laboratory_id
        && target.asset_id == source.asset_id
        && target.quantity_unit_id == source.quantity_unit_id
        && target.batch_number == source.batch_number
        && target.location_id == source.location_id
        && target.status == source.status
    {
        Ok(())
    } else {
        Err(InventoryItemError::ValidationError(
            "Source inventory items must match target asset, unit, batch, location, and status"
                .into(),
        ))
    }
}
