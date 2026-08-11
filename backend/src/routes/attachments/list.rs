use super::model::AttachmentResponse;
use super::queries::{
    count_laboratory_attachments, fetch_asset_attachments, fetch_asset_laboratory_id,
    fetch_inventory_item_attachments, fetch_inventory_item_laboratory_id,
    fetch_laboratory_attachments,
};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::access_control::{AssetPathId, InventoryItemPathId};
use crate::domain::{AssetId, InventoryItemId};
use crate::routes::{PaginatedResponse, Pagination, PaginationError};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum ListAttachmentError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl From<PaginationError> for ListAttachmentError {
    fn from(error: PaginationError) -> Self {
        Self::ValidationError(error.to_string())
    }
}

impl std::fmt::Debug for ListAttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListAttachmentError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListAttachmentError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ListAttachmentError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListAttachmentError::NotFound(_) => StatusCode::NOT_FOUND,
            ListAttachmentError::ConflictError(_) => StatusCode::CONFLICT,
            ListAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "List asset attachments",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, asset_id=%asset_id)
)]
pub async fn list_asset_attachments(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    asset_id: AssetPathId,
) -> Result<HttpResponse, ListAttachmentError> {
    let actor = laboratory_context.authorization_actor();
    let asset_id: AssetId = asset_id.into_inner().into();
    let laboratory_id = fetch_asset_laboratory_id(&pool, asset_id)
        .await?
        .ok_or_else(|| ListAttachmentError::NotFound("Asset not found".into()))?;
    if laboratory_context.laboratory_id() != laboratory_id {
        return Err(ListAttachmentError::NotFound("Asset not found".into()));
    }
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::AttachmentAssignment,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListAttachmentError::Forbidden(
            "You do not have permission to view attachments for this asset".into(),
        ));
    }
    let include_internal = validate_permission(
        &pool,
        &actor,
        ResourceType::AttachmentAssignment,
        Action::BrowseInternal(laboratory_id.into()),
    )
    .await?;
    let attachments: Vec<_> = fetch_asset_attachments(&pool, asset_id, include_internal)
        .await?
        .into_iter()
        .map(AttachmentResponse::from)
        .collect();
    Ok(HttpResponse::Ok().json(attachments))
}

#[tracing::instrument(
    name = "List inventory item attachments",
    skip(pool),
    fields(actor_user_id=%laboratory_context.actor().user_id, inventory_item_id=%inventory_item_id)
)]
pub async fn list_inventory_item_attachments(
    laboratory_context: LaboratoryContext,
    pool: web::Data<PgPool>,
    inventory_item_id: InventoryItemPathId,
) -> Result<HttpResponse, ListAttachmentError> {
    let actor = laboratory_context.authorization_actor();
    let inventory_item_id: InventoryItemId = inventory_item_id.into_inner().into();
    let laboratory_id = fetch_inventory_item_laboratory_id(&pool, inventory_item_id)
        .await?
        .ok_or_else(|| ListAttachmentError::NotFound("Inventory item not found".into()))?;
    if laboratory_context.laboratory_id() != laboratory_id {
        return Err(ListAttachmentError::NotFound(
            "Inventory item not found".into(),
        ));
    }
    if !validate_permission(
        &pool,
        &actor,
        ResourceType::AttachmentAssignment,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListAttachmentError::Forbidden(
            "You do not have permission to view attachments for this inventory item".into(),
        ));
    }
    let include_internal = validate_permission(
        &pool,
        &actor,
        ResourceType::AttachmentAssignment,
        Action::BrowseInternal(laboratory_id.into()),
    )
    .await?;
    let attachments: Vec<_> =
        fetch_inventory_item_attachments(&pool, inventory_item_id, include_internal)
            .await?
            .into_iter()
            .map(AttachmentResponse::from)
            .collect();
    Ok(HttpResponse::Ok().json(attachments))
}

#[tracing::instrument(
    name = "List laboratory attachments",
    skip(pool, pagination),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn list_laboratory_attachments(
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
    pagination: web::Query<Pagination>,
) -> Result<HttpResponse, ListAttachmentError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    if !validate_permission(
        &pool,
        actor,
        ResourceType::AttachmentAssignment,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListAttachmentError::Forbidden(
            "You do not have permission to view attachments for this laboratory".into(),
        ));
    }
    let include_internal = validate_permission(
        &pool,
        actor,
        ResourceType::AttachmentAssignment,
        Action::BrowseInternal(laboratory_id.into()),
    )
    .await?;
    let pagination = pagination.into_inner();
    let total = count_laboratory_attachments(&pool, laboratory_id, include_internal).await?;
    let attachments = fetch_laboratory_attachments(
        &pool,
        laboratory_id,
        include_internal,
        pagination.limit()?,
        pagination.offset()?,
    )
    .await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(
        attachments
            .into_iter()
            .map(AttachmentResponse::from)
            .collect::<Vec<_>>(),
        &pagination,
        total,
    )?))
}
