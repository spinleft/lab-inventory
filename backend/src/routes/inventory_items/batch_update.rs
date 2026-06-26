use super::model::{
    InventoryItemError, InventoryItemPatch, InventoryItemResponse, actor_for_user,
    apply_inventory_item_patch, fetch_inventory_items_for_update, parse_nullable_string,
    parse_nullable_uuid, record_inventory_item_audit, record_update_transaction,
    update_inventory_item_rollback_details, validate_requested_ids, validate_write_permission,
};
use crate::audit::AuditAction;
use crate::domain::{NullableUpdate, UserId};
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    inventory_item_ids: Vec<Uuid>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    batch_number: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    location_id: Option<Option<Uuid>>,
    status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    public_notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    internal_notes: Option<Option<String>>,
}

impl From<&JsonData> for InventoryItemPatch {
    fn from(value: &JsonData) -> Self {
        Self {
            serial_number: None,
            batch_number: parse_nullable_string(value.batch_number.clone()),
            quantity_on_hand: None,
            quantity_allocated: None,
            quantity_unit_id: None,
            location_id: parse_nullable_uuid(value.location_id),
            status: value.status.clone(),
            public_notes: parse_nullable_string(value.public_notes.clone()),
            internal_notes: parse_nullable_string(value.internal_notes.clone()),
        }
    }
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[tracing::instrument(
    name = "Batch update inventory items",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id)
)]
pub async fn batch_update_inventory_items(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, InventoryItemError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let payload = payload.into_inner();
    let patch = InventoryItemPatch::from(&payload);
    validate_patch_has_updates(&patch)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing_items =
        fetch_inventory_items_for_update(&mut transaction, &payload.inventory_item_ids).await?;
    validate_requested_ids(&payload.inventory_item_ids, &existing_items)?;
    for item in &existing_items {
        validate_write_permission(&actor, item.laboratory_id)?;
    }

    let mut updated_items = Vec::with_capacity(existing_items.len());
    for existing in existing_items {
        let updated =
            apply_inventory_item_patch(&mut transaction, &existing, patch.clone()).await?;
        record_update_transaction(
            &mut transaction,
            &actor,
            &existing,
            &updated,
            "batch_update",
        )
        .await?;
        record_inventory_item_audit(
            &mut transaction,
            &actor,
            AuditAction::Update,
            updated.inventory_item_id,
            update_inventory_item_rollback_details(&existing),
        )
        .await?;
        updated_items.push(updated);
    }

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to batch update inventory items.")?;
    let response = updated_items
        .into_iter()
        .map(InventoryItemResponse::from)
        .collect::<Vec<_>>();
    Ok(HttpResponse::Ok().json(response))
}

fn validate_patch_has_updates(patch: &InventoryItemPatch) -> Result<(), InventoryItemError> {
    if !matches!(patch.batch_number, NullableUpdate::Unchanged)
        || !matches!(patch.location_id, NullableUpdate::Unchanged)
        || patch.status.is_some()
        || !matches!(patch.public_notes, NullableUpdate::Unchanged)
        || !matches!(patch.internal_notes, NullableUpdate::Unchanged)
    {
        Ok(())
    } else {
        Err(InventoryItemError::ValidationError(
            "Batch update requires at least one update field".into(),
        ))
    }
}
