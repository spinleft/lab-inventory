use super::error::AttachmentError;
use super::model::{
    AttachmentTarget, ListLaboratoryAttachmentsQuery, actor_for_user, fetch_asset_target,
    fetch_attachments_for_target, fetch_inventory_item_target, fetch_laboratory_attachment_count,
    fetch_laboratory_attachments, response_vec, validate_read_permission,
};
use crate::domain::{LaboratoryId, UserId};
use crate::routes::PaginatedResponse;
use actix_web::{HttpResponse, web};
use anyhow::anyhow;
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(
    name = "List asset attachments",
    skip(pool),
    fields(actor_user_id=%actor_user_id, asset_id=%asset_id)
)]
pub async fn list_asset_attachments(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<Uuid>,
) -> Result<HttpResponse, AttachmentError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let asset_id = asset_id.into_inner();
    let target = fetch_asset_target(&pool, asset_id)
        .await?
        .ok_or_else(|| AttachmentError::NotFound("Asset not found".into()))?;
    let laboratory_id = LaboratoryId::parse(target.laboratory_id)
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    let include_internal = validate_read_permission(&actor, &laboratory_id)?;
    let rows =
        fetch_attachments_for_target(&pool, AttachmentTarget::Asset(asset_id), include_internal)
            .await?;
    Ok(HttpResponse::Ok().json(response_vec(rows)))
}

#[tracing::instrument(
    name = "List inventory item attachments",
    skip(pool),
    fields(actor_user_id=%actor_user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn list_inventory_item_attachments(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    inventory_item_id: web::Path<Uuid>,
) -> Result<HttpResponse, AttachmentError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let inventory_item_id = inventory_item_id.into_inner();
    let target = fetch_inventory_item_target(&pool, inventory_item_id)
        .await?
        .ok_or_else(|| AttachmentError::NotFound("Inventory item not found".into()))?;
    let laboratory_id = LaboratoryId::parse(target.laboratory_id)
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    let include_internal = validate_read_permission(&actor, &laboratory_id)?;
    let rows = fetch_attachments_for_target(
        &pool,
        AttachmentTarget::InventoryItem(inventory_item_id),
        include_internal,
    )
    .await?;
    Ok(HttpResponse::Ok().json(response_vec(rows)))
}

#[tracing::instrument(
    name = "List laboratory attachments",
    skip(pool, query),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn list_laboratory_attachments(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    query: web::Query<ListLaboratoryAttachmentsQuery>,
) -> Result<HttpResponse, AttachmentError> {
    let actor = actor_for_user(&pool, actor_user_id).await?;
    let laboratory_id = LaboratoryId::parse(laboratory_id.into_inner())
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    let include_internal = validate_read_permission(&actor, &laboratory_id)?;
    let total = fetch_laboratory_attachment_count(&pool, laboratory_id, include_internal).await?;
    let limit = query.pagination.limit()?;
    let offset = query.pagination.offset()?;
    let rows =
        fetch_laboratory_attachments(&pool, laboratory_id, include_internal, limit, offset).await?;
    Ok(HttpResponse::Ok().json(PaginatedResponse::new(
        response_vec(rows),
        &query.pagination,
        total,
    )?))
}
