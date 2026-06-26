use super::model::{
    InventoryItemError, InventoryItemResponse, actor_for_user,
    create_inventory_items_rollback_details, fetch_asset_for_inventory_for_update,
    insert_inventory_item, next_serial_numbers, normalize_serial_numbers,
    record_inventory_item_audit, record_inventory_transaction, resolve_asset_quantity_unit,
    validate_location, validate_quantities, validate_status, validate_write_permission,
};
use crate::audit::AuditAction;
use crate::domain::{AssetTrackingMode, AttachmentClaim, UserId};
use crate::routes::attachments::{
    AttachmentClaimInput, AttachmentError, claim_inventory_item_attachments,
};
use actix_web::{HttpResponse, web};
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    serial_numbers: Option<Vec<String>>,
    count: Option<i64>,
    batch_number: Option<String>,
    quantity_on_hand: Option<f64>,
    quantity_allocated: Option<f64>,
    quantity_unit_id: Option<Uuid>,
    location_id: Option<Uuid>,
    status: Option<String>,
    public_notes: Option<String>,
    internal_notes: Option<String>,
    attachments: Option<Vec<AttachmentClaimInput>>,
}

#[tracing::instrument(
    name = "Create inventory items",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, asset_id=%asset_id)
)]
pub async fn create_inventory_items(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, InventoryItemError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let mut payload = payload.into_inner();
    let attachment_claims =
        parse_attachment_claims(payload.attachments.take().unwrap_or_default())?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let asset = fetch_asset_for_inventory_for_update(&mut transaction, asset_id.into_inner())
        .await?
        .ok_or_else(|| InventoryItemError::NotFound("Asset not found".into()))?;
    let laboratory_id = validate_write_permission(&actor, asset.laboratory_id)?;
    if let Some(location_id) = payload.location_id {
        validate_location(&mut transaction, asset.laboratory_id, location_id).await?;
    }
    let status =
        validate_status(payload.status.clone())?.unwrap_or_else(|| "available".to_string());
    let tracking_mode = AssetTrackingMode::parse(&asset.tracking_mode)
        .map_err(InventoryItemError::ValidationError)?;

    let created = match tracking_mode {
        AssetTrackingMode::Serialized => {
            create_serialized_items(&mut transaction, &asset, payload, &status).await?
        }
        AssetTrackingMode::Quantity => {
            create_quantity_item(&mut transaction, &asset, payload, &status).await?
        }
    };
    if !attachment_claims.is_empty() {
        if created.len() != 1 {
            return Err(InventoryItemError::ValidationError(
                "attachments can only be supplied when exactly one inventory item is created"
                    .into(),
            ));
        }
        claim_inventory_item_attachments(
            &mut transaction,
            &actor,
            laboratory_id,
            created[0].inventory_item_id,
            &attachment_claims,
        )
        .await
        .map_err(map_attachment_error)?;
    }

    for item in &created {
        record_inventory_transaction(
            &mut transaction,
            &actor,
            item,
            "create",
            item.quantity_on_hand,
            item.quantity_allocated,
            None,
            item.location_id,
            json!({
                "operation": "create",
                "inventory_item": item,
            }),
        )
        .await?;
    }
    record_inventory_item_audit(
        &mut transaction,
        &actor,
        AuditAction::Create,
        created[0].inventory_item_id,
        create_inventory_items_rollback_details(&created),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to create inventory items.")?;

    let response = created
        .into_iter()
        .map(InventoryItemResponse::from)
        .collect::<Vec<_>>();
    Ok(HttpResponse::Created().json(response))
}

fn map_attachment_error(error: AttachmentError) -> InventoryItemError {
    match error {
        AttachmentError::ValidationError(message) => InventoryItemError::ValidationError(message),
        AttachmentError::Forbidden(message) => InventoryItemError::Forbidden(message),
        AttachmentError::NotFound(message) => InventoryItemError::ValidationError(message),
        AttachmentError::ConflictError(message) => InventoryItemError::ConflictError(message),
        AttachmentError::UnexpectedError(error) => InventoryItemError::UnexpectedError(error),
    }
}

fn parse_attachment_claims(
    claims: Vec<AttachmentClaimInput>,
) -> Result<Vec<AttachmentClaim>, InventoryItemError> {
    claims
        .into_iter()
        .map(AttachmentClaim::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(InventoryItemError::ValidationError)
}

async fn create_serialized_items(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    asset: &super::model::AssetForInventoryRow,
    payload: JsonData,
    status: &str,
) -> Result<Vec<super::model::InventoryItemRow>, InventoryItemError> {
    if payload.quantity_on_hand.is_some()
        || payload.quantity_allocated.is_some()
        || payload.quantity_unit_id.is_some()
    {
        return Err(InventoryItemError::ValidationError(
            "Serialized inventory items cannot specify quantity fields".into(),
        ));
    }
    let serial_numbers = match (payload.serial_numbers, payload.count) {
        (Some(serial_numbers), None) => normalize_serial_numbers(serial_numbers)?,
        (None, Some(count)) => next_serial_numbers(transaction, asset.asset_id, count).await?,
        (Some(_), Some(_)) => {
            return Err(InventoryItemError::ValidationError(
                "serialized creation accepts serial_numbers or count, not both".into(),
            ));
        }
        (None, None) => {
            return Err(InventoryItemError::ValidationError(
                "serialized creation requires serial_numbers or count".into(),
            ));
        }
    };

    let mut created = Vec::with_capacity(serial_numbers.len());
    for serial_number in serial_numbers {
        created.push(
            insert_inventory_item(
                transaction,
                asset.asset_id,
                asset.laboratory_id,
                "serialized",
                Some(&serial_number),
                payload.batch_number.as_deref(),
                1.0,
                0.0,
                asset.default_unit_id,
                payload.location_id,
                status,
                payload.public_notes.as_deref(),
                payload.internal_notes.as_deref(),
            )
            .await?,
        );
    }
    Ok(created)
}

async fn create_quantity_item(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    asset: &super::model::AssetForInventoryRow,
    payload: JsonData,
    status: &str,
) -> Result<Vec<super::model::InventoryItemRow>, InventoryItemError> {
    if payload.serial_numbers.is_some() || payload.count.is_some() {
        return Err(InventoryItemError::ValidationError(
            "Quantity-tracked inventory items cannot specify serial_numbers or count".into(),
        ));
    }
    let quantity_on_hand = payload.quantity_on_hand.ok_or_else(|| {
        InventoryItemError::ValidationError(
            "Quantity-tracked inventory items require quantity_on_hand".into(),
        )
    })?;
    let quantity_allocated = payload.quantity_allocated.unwrap_or(0.0);
    validate_quantities(quantity_on_hand, quantity_allocated)?;
    let quantity_unit_id =
        resolve_asset_quantity_unit(payload.quantity_unit_id, asset.default_unit_id)?;

    let created = insert_inventory_item(
        transaction,
        asset.asset_id,
        asset.laboratory_id,
        "quantity",
        None,
        payload.batch_number.as_deref(),
        quantity_on_hand,
        quantity_allocated,
        quantity_unit_id,
        payload.location_id,
        status,
        payload.public_notes.as_deref(),
        payload.internal_notes.as_deref(),
    )
    .await?;
    Ok(vec![created])
}
