use super::model::{
    InventoryItemError, InventoryItemResponse, actor_for_user, convert_quantity_between_units,
    find_quantity_aggregate_for_update, insert_inventory_item, parse_nullable_string,
    parse_nullable_uuid, record_inventory_item_audit, record_inventory_transaction,
    resolve_asset_quantity_unit, set_quantity_on_hand, split_inventory_item_rollback_details,
    validate_location, validate_quantity_item, validate_status, validate_write_permission,
};
use crate::audit::AuditAction;
use crate::domain::UserId;
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    quantity: f64,
    quantity_unit_id: Option<Uuid>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    batch_number: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    location_id: Option<Option<Uuid>>,
    status: Option<String>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
}

#[derive(Serialize)]
struct SplitInventoryItemResponse {
    source: InventoryItemResponse,
    target: InventoryItemResponse,
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[tracing::instrument(
    name = "Split inventory item",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn split_inventory_item(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, InventoryItemError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let payload = payload.into_inner();
    if payload.quantity <= 0.0 {
        return Err(InventoryItemError::ValidationError(
            "quantity must be positive".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let source_before = super::model::fetch_inventory_item_for_update(
        &mut transaction,
        inventory_item_id.into_inner(),
    )
    .await?
    .ok_or_else(|| InventoryItemError::NotFound("Inventory item not found".into()))?;
    validate_write_permission(&actor, source_before.laboratory_id)?;
    validate_quantity_item(&source_before)?;

    let available_quantity = source_before.quantity_on_hand - source_before.quantity_allocated;
    if payload.quantity > available_quantity {
        return Err(InventoryItemError::ValidationError(
            "Split quantity cannot exceed unallocated quantity".into(),
        ));
    }

    let target_status =
        validate_status(payload.status)?.unwrap_or_else(|| source_before.status.clone());
    let target_batch =
        parse_nullable_string(payload.batch_number).resolve(source_before.batch_number.clone());
    let target_location_id =
        parse_nullable_uuid(payload.location_id).resolve(source_before.location_id);
    if let Some(location_id) = target_location_id {
        validate_location(&mut transaction, source_before.laboratory_id, location_id).await?;
    }
    let target_unit_id = resolve_asset_quantity_unit(
        payload.quantity_unit_id,
        source_before.asset_default_unit_id,
    )?;
    let target_quantity = convert_quantity_between_units(
        &mut transaction,
        source_before.quantity_unit_id,
        target_unit_id,
        payload.quantity,
    )
    .await?;

    if target_batch == source_before.batch_number
        && target_location_id == source_before.location_id
        && target_status == source_before.status
        && target_unit_id == source_before.quantity_unit_id
    {
        return Err(InventoryItemError::ValidationError(
            "Split target must differ by batch, location, status, or unit".into(),
        ));
    }

    let target_before = find_quantity_aggregate_for_update(
        &mut transaction,
        source_before.laboratory_id,
        source_before.asset_id,
        target_batch.as_deref(),
        target_location_id,
        &target_status,
        target_unit_id,
        Some(source_before.inventory_item_id),
    )
    .await?;

    let source_after = set_quantity_on_hand(
        &mut transaction,
        source_before.inventory_item_id,
        source_before.quantity_on_hand - payload.quantity,
    )
    .await?;
    let target_after = if let Some(target) = target_before.as_ref() {
        super::model::add_quantities_to_item(
            &mut transaction,
            target.inventory_item_id,
            target_quantity,
            0.0,
        )
        .await?
    } else {
        insert_inventory_item(
            &mut transaction,
            source_before.asset_id,
            source_before.laboratory_id,
            "quantity",
            None,
            target_batch.as_deref(),
            target_quantity,
            0.0,
            target_unit_id,
            target_location_id,
            &target_status,
            payload
                .public_notes
                .as_deref()
                .or(source_before.public_notes.as_deref()),
            payload
                .internal_notes
                .as_deref()
                .or(source_before.internal_notes.as_deref()),
        )
        .await?
    };

    record_inventory_transaction(
        &mut transaction,
        &actor,
        &source_after,
        "adjust",
        -payload.quantity,
        0.0,
        source_before.location_id,
        source_after.location_id,
        json!({
            "operation": "split",
            "role": "source",
            "source_before": source_before,
            "source_after": source_after,
            "target_inventory_item_id": target_after.inventory_item_id,
        }),
    )
    .await?;
    record_inventory_transaction(
        &mut transaction,
        &actor,
        &target_after,
        if target_before.is_some() {
            "adjust"
        } else {
            "create"
        },
        target_quantity,
        0.0,
        None,
        target_after.location_id,
        json!({
            "operation": "split",
            "role": "target",
            "source_inventory_item_id": source_after.inventory_item_id,
            "target_before": target_before,
            "target_after": target_after,
        }),
    )
    .await?;
    record_inventory_item_audit(
        &mut transaction,
        &actor,
        AuditAction::Adjust,
        source_after.inventory_item_id,
        split_inventory_item_rollback_details(
            &source_before,
            target_before.as_ref(),
            &target_after,
        ),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to split inventory item.")?;

    Ok(HttpResponse::Ok().json(SplitInventoryItemResponse {
        source: InventoryItemResponse::from(source_after),
        target: InventoryItemResponse::from(target_after),
    }))
}
