use super::model::{
    InventoryItemError, InventoryItemResponse, actor_for_user, fetch_inventory_item,
    validate_read_permission,
};
use crate::domain::UserId;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(
    name = "Get an inventory item",
    skip(pool),
    fields(actor_user_id=%actor_user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn get_inventory_item(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
) -> Result<HttpResponse, InventoryItemError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let item = fetch_inventory_item(&pool, inventory_item_id.into_inner())
        .await?
        .ok_or_else(|| InventoryItemError::NotFound("Inventory item not found".into()))?;
    let laboratory_id = validate_read_permission(&actor, item.laboratory_id)?;
    let include_internal_notes = actor.can_read_laboratory_resource(&laboratory_id);
    Ok(HttpResponse::Ok().json(InventoryItemResponse::from_row(
        item,
        include_internal_notes,
    )))
}
