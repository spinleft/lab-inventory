use super::model::InventoryItemResponse;
use super::queries::fetch_inventory_item;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::domain::InventoryItemId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum GetInventoryItemError {
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for GetInventoryItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for GetInventoryItemError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetInventoryItemError::Forbidden(_) => StatusCode::FORBIDDEN,
            GetInventoryItemError::NotFound(_) => StatusCode::NOT_FOUND,
            GetInventoryItemError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Get an inventory item",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn get_inventory_item(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    inventory_item_id: InventoryItemId,
) -> Result<HttpResponse, GetInventoryItemError> {
    let actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::InventoryItem,
        Action::Read(inventory_item_id.into()),
    )
    .await?
    {
        return Err(GetInventoryItemError::Forbidden(
            "You are not allowed to get this inventory item.".into(),
        ));
    }

    let item = fetch_inventory_item(&pool, inventory_item_id.into())
        .await?
        .ok_or(GetInventoryItemError::NotFound(
            "Inventory item not found".into(),
        ))?;
    let include_internal_notes = validate_permission(
        &pool,
        &actor,
        ResourceType::InventoryItem,
        Action::BrowseInternal(item.laboratory_id),
    )
    .await?;

    Ok(HttpResponse::Ok().json(InventoryItemResponse::from_row(
        item,
        include_internal_notes,
    )))
}
