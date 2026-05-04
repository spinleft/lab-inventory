use super::helpers::fetch_inventory_item;
use super::model::InventoryItemResponse;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Get an inventory item", skip(pool), fields(user_id=%user_id, inventory_item_id=%inventory_item_id))]
pub async fn get_inventory_item(
    user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let item = fetch_inventory_item(pool.get_ref(), inventory_item_id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(InventoryItemResponse::from_row(item, &actor)))
}
