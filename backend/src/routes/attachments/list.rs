use super::model::{AttachmentResponse, AttachmentRow};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::domain::{AssetId, InventoryItemId, LaboratoryId, UserId};
use crate::routes::{PaginatedResponse, Pagination, PaginationError};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum ListAttachmentError {
    #[error("{0}")]
    ValidationError(String),
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
            ListAttachmentError::NotFound(_) => StatusCode::NOT_FOUND,
            ListAttachmentError::ConflictError(_) => StatusCode::CONFLICT,
            ListAttachmentError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "List asset attachments",
    skip(pool),
    fields(actor_user_id=%actor_user_id, asset_id=%asset_id)
)]
pub async fn list_asset_attachments(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<Uuid>,
) -> Result<HttpResponse, ListAttachmentError> {
    let asset_id: AssetId = asset_id.into_inner().into();
    let laboratory_id = fetch_asset_laboratory_id(&pool, asset_id).await?;
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::AttachmentAssignment,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListAttachmentError::ValidationError(
            "You do not have permission to view attachments for this asset".into(),
        ));
    }
    let include_internal = validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::AttachmentAssignment,
        Action::BrowseInternal(laboratory_id.into()),
    )
    .await?;
    let attachments: Vec<_> = fetch_asset_attachments(&pool, asset_id, include_internal).await?;
    Ok(HttpResponse::Ok().json(attachments))
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
) -> Result<HttpResponse, ListAttachmentError> {
    let inventory_item_id: InventoryItemId = inventory_item_id.into_inner().into();
    let laboratory_id = fetch_inventory_item_laboratory_id(&pool, inventory_item_id).await?;
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::AttachmentAssignment,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListAttachmentError::ValidationError(
            "You do not have permission to view attachments for this inventory item".into(),
        ));
    }
    let include_internal = validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::AttachmentAssignment,
        Action::BrowseInternal(laboratory_id.into()),
    )
    .await?;
    let attachments: Vec<_> = fetch_inventory_item_attachments(&pool, inventory_item_id, include_internal).await?;
    Ok(HttpResponse::Ok().json(attachments))
}

#[tracing::instrument(
    name = "List laboratory attachments",
    skip(pool, pagination),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn list_laboratory_attachments(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    pagination: web::Query<Pagination>,
) -> Result<HttpResponse, ListAttachmentError> {
    let laboratory_id: LaboratoryId = laboratory_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::AttachmentAssignment,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListAttachmentError::ValidationError(
            "You do not have permission to view attachments for this laboratory".into(),
        ));
    }
    let include_internal = validate_permission(
        &pool,
        &actor_user_id,
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
        attachments.into_iter()
            .map(AttachmentResponse::from)
            .collect::<Vec<_>>(),
        &pagination,
        total,
    )?))
}

#[derive(sqlx::FromRow)]
struct LaboratoryIdRow {
    pub laboratory_id: Uuid,
}

async fn fetch_asset_laboratory_id(
    pool: &PgPool,
    asset_id: AssetId,
) -> Result<LaboratoryId, ListAttachmentError> {
    sqlx::query_as!(
        LaboratoryIdRow,
        r#"
        SELECT laboratory_id
        FROM assets
        WHERE asset_id = $1
        FOR UPDATE
        "#,
        Uuid::from(asset_id)
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| ListAttachmentError::UnexpectedError(e.into()))?
    .map(|row| row.laboratory_id.into())
    .ok_or_else(|| ListAttachmentError::NotFound("Asset not found".into()))
}

async fn fetch_inventory_item_laboratory_id(
    pool: &PgPool,
    inventory_item_id: InventoryItemId,
) -> Result<LaboratoryId, ListAttachmentError> {
    sqlx::query_as!(
        LaboratoryIdRow,
        r#"
        SELECT laboratory_id
        FROM asset_inventory_items
        WHERE inventory_item_id = $1
        FOR UPDATE
        "#,
        Uuid::from(inventory_item_id)
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| ListAttachmentError::UnexpectedError(e.into()))?
    .map(|row| row.laboratory_id.into())
    .ok_or_else(|| ListAttachmentError::NotFound("Inventory item not found".into()))
}

async fn fetch_asset_attachments(
    pool: &PgPool,
    asset_id: AssetId,
    include_internal: bool,
) -> Result<Vec<AttachmentRow>, ListAttachmentError> {
    sqlx::query_as!(
        AttachmentRow,
        r#"
        SELECT
            assignments.attachment_id,
            assignments.laboratory_id,
            assignments.file_id,
            assignments.asset_id,
            assignments.inventory_item_id,
            assignments.display_name,
            assignments.description,
            assignments.is_public,
            assignments.assigned_by_user_id,
            assignments.created_at,
            assignments.updated_at,
            files.storage_backend,
            files.storage_key,
            files.original_file_name,
            files.mime_type,
            files.file_size_bytes,
            files.sha256_hex,
            files.uploaded_by_user_id,
            files.created_at AS file_created_at
        FROM asset_attachment_assignments AS assignments
        JOIN files ON files.file_id = assignments.file_id
        WHERE assignments.asset_id = $1
        AND ($2 OR assignments.is_public = 'true')
        ORDER BY assignments.created_at DESC, assignments.attachment_id
        "#,
        Uuid::from(asset_id),
        include_internal
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ListAttachmentError::UnexpectedError(e.into()))
}

async fn fetch_inventory_item_attachments(
    pool: &PgPool,
    inventory_item_id: InventoryItemId,
    include_internal: bool,
) -> Result<Vec<AttachmentRow>, ListAttachmentError> {
    sqlx::query_as!(
        AttachmentRow,
        r#"
        SELECT
            assignments.attachment_id,
            assignments.laboratory_id,
            assignments.file_id,
            assignments.asset_id,
            assignments.inventory_item_id,
            assignments.display_name,
            assignments.description,
            assignments.is_public,
            assignments.assigned_by_user_id,
            assignments.created_at,
            assignments.updated_at,
            files.storage_backend,
            files.storage_key,
            files.original_file_name,
            files.mime_type,
            files.file_size_bytes,
            files.sha256_hex,
            files.uploaded_by_user_id,
            files.created_at AS file_created_at
        FROM asset_attachment_assignments AS assignments
        JOIN files ON files.file_id = assignments.file_id
        WHERE assignments.inventory_item_id = $1
        AND ($2 OR assignments.is_public = 'true')
        ORDER BY assignments.created_at DESC, assignments.attachment_id
        "#,
        Uuid::from(inventory_item_id),
        include_internal
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ListAttachmentError::UnexpectedError(e.into()))
}

async fn count_laboratory_attachments(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    include_internal: bool,
) -> Result<i64, ListAttachmentError> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM asset_attachment_assignments AS assignments
        WHERE assignments.laboratory_id = $1
        AND ($2 OR assignments.is_public = 'true')
        "#,
        Uuid::from(laboratory_id),
        include_internal
    )
    .fetch_one(pool)
    .await
    .map_err(|e| ListAttachmentError::UnexpectedError(e.into()))
}

async fn fetch_laboratory_attachments(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    include_internal: bool,
    limit: i64,
    offset: i64,
) -> Result<Vec<AttachmentRow>, ListAttachmentError> {
    sqlx::query_as!(
        AttachmentRow,
        r#"
        SELECT
            assignments.attachment_id,
            assignments.laboratory_id,
            assignments.file_id,
            assignments.asset_id,
            assignments.inventory_item_id,
            assignments.display_name,
            assignments.description,
            assignments.is_public,
            assignments.assigned_by_user_id,
            assignments.created_at,
            assignments.updated_at,
            files.storage_backend,
            files.storage_key,
            files.original_file_name,
            files.mime_type,
            files.file_size_bytes,
            files.sha256_hex,
            files.uploaded_by_user_id,
            files.created_at AS file_created_at
        FROM asset_attachment_assignments AS assignments
        JOIN files ON files.file_id = assignments.file_id
        WHERE assignments.laboratory_id = $1
        AND ($2 OR assignments.is_public = 'true')
        ORDER BY assignments.created_at DESC, assignments.attachment_id
        LIMIT $3 OFFSET $4
        "#,
        Uuid::from(laboratory_id),
        include_internal,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ListAttachmentError::UnexpectedError(e.into()))
}
