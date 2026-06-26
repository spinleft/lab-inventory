use super::model::{
    InventoryItemError, InventoryItemPatch, InventoryItemResponse, actor_for_user,
    apply_inventory_item_patch, fetch_inventory_item_for_update, parse_nullable_string,
    parse_nullable_uuid, record_inventory_item_audit, update_inventory_item_rollback_details,
    validate_write_permission,
};
use crate::audit::AuditAction;
use crate::domain::UserId;
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    serial_number: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    batch_number: Option<Option<String>>,
    quantity_on_hand: Option<f64>,
    quantity_allocated: Option<f64>,
    quantity_unit_id: Option<Uuid>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    location_id: Option<Option<Uuid>>,
    status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    public_notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    internal_notes: Option<Option<String>>,
}

impl From<JsonData> for InventoryItemPatch {
    fn from(value: JsonData) -> Self {
        Self {
            serial_number: value.serial_number,
            batch_number: parse_nullable_string(value.batch_number),
            quantity_on_hand: value.quantity_on_hand,
            quantity_allocated: value.quantity_allocated,
            quantity_unit_id: value.quantity_unit_id,
            location_id: parse_nullable_uuid(value.location_id),
            status: value.status,
            public_notes: parse_nullable_string(value.public_notes),
            internal_notes: parse_nullable_string(value.internal_notes),
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
    name = "Update an inventory item",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn update_inventory_item(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, InventoryItemError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing =
        fetch_inventory_item_for_update(&mut transaction, inventory_item_id.into_inner())
            .await?
            .ok_or_else(|| InventoryItemError::NotFound("Inventory item not found".into()))?;
    validate_write_permission(&actor, existing.laboratory_id)?;

    let updated =
        apply_inventory_item_patch(&mut transaction, &existing, payload.into_inner().into())
            .await?;
    record_inventory_item_audit(
        &mut transaction,
        &actor,
        AuditAction::Update,
        updated.inventory_item_id,
        update_inventory_item_rollback_details(&existing),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update an inventory item.")?;

    Ok(HttpResponse::Ok().json(InventoryItemResponse::from(updated)))
}
