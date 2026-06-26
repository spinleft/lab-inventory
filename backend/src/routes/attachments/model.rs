use super::error::AttachmentError;
use crate::access_control::{Actor, get_actor};
use crate::domain::{
    AttachmentClaim, AttachmentDescription, AttachmentDisplayName, AttachmentUploadId,
    AttachmentVisibility, LaboratoryId, UserId,
};
use crate::routes::Pagination;
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentClaimInput {
    pub(crate) upload_id: Uuid,
    pub(crate) display_name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) visibility: Option<String>,
}

impl TryFrom<AttachmentClaimInput> for AttachmentClaim {
    type Error = String;

    fn try_from(value: AttachmentClaimInput) -> Result<Self, Self::Error> {
        Ok(Self::new(
            AttachmentUploadId::parse(value.upload_id)?,
            value
                .display_name
                .map(AttachmentDisplayName::parse)
                .transpose()?,
            value
                .description
                .map(AttachmentDescription::parse_optional)
                .transpose()?
                .flatten(),
            value
                .visibility
                .as_deref()
                .map(AttachmentVisibility::parse)
                .transpose()?,
        ))
    }
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub(crate) struct AttachmentRow {
    pub(crate) attachment_id: Uuid,
    pub(crate) laboratory_id: Uuid,
    pub(crate) asset_id: Option<Uuid>,
    pub(crate) inventory_item_id: Option<Uuid>,
    pub(crate) display_name: String,
    pub(crate) original_file_name: String,
    pub(crate) description: Option<String>,
    pub(crate) mime_type: Option<String>,
    pub(crate) file_size_bytes: i64,
    pub(crate) sha256_hex: String,
    pub(crate) storage_backend: String,
    pub(crate) storage_key: String,
    pub(crate) visibility: String,
    pub(crate) uploaded_by_user_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Clone, sqlx::FromRow)]
pub(crate) struct DeletedAttachmentRow {
    pub(crate) attachment_id: Uuid,
    pub(crate) storage_key: String,
}

#[derive(Serialize)]
pub(super) struct AttachmentResponse {
    attachment_id: Uuid,
    laboratory_id: Uuid,
    asset_id: Option<Uuid>,
    inventory_item_id: Option<Uuid>,
    display_name: String,
    original_file_name: String,
    description: Option<String>,
    mime_type: Option<String>,
    file_size_bytes: i64,
    sha256_hex: String,
    visibility: String,
    uploaded_by_user_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<AttachmentRow> for AttachmentResponse {
    fn from(row: AttachmentRow) -> Self {
        Self {
            attachment_id: row.attachment_id,
            laboratory_id: row.laboratory_id,
            asset_id: row.asset_id,
            inventory_item_id: row.inventory_item_id,
            display_name: row.display_name,
            original_file_name: row.original_file_name,
            description: row.description,
            mime_type: row.mime_type,
            file_size_bytes: row.file_size_bytes,
            sha256_hex: row.sha256_hex,
            visibility: row.visibility,
            uploaded_by_user_id: row.uploaded_by_user_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct AttachmentUploadResponse {
    upload_id: Uuid,
    laboratory_id: Uuid,
    original_file_name: String,
    mime_type: Option<String>,
    file_size_bytes: i64,
    sha256_hex: String,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub(super) struct AttachmentUploadRow {
    pub(super) upload_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) storage_backend: String,
    pub(super) storage_key: String,
    pub(super) original_file_name: String,
    pub(super) mime_type: Option<String>,
    pub(super) file_size_bytes: i64,
    pub(super) sha256_hex: String,
    pub(super) uploaded_by_user_id: Option<Uuid>,
    pub(super) expires_at: DateTime<Utc>,
    pub(super) consumed_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
pub(super) struct TargetRow {
    pub(super) laboratory_id: Uuid,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ListLaboratoryAttachmentsQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
}

pub(super) enum AttachmentTarget {
    Asset(Uuid),
    InventoryItem(Uuid),
}

pub(super) fn attachment_columns() -> &'static str {
    r#"
    attachment_id,
    laboratory_id,
    asset_id,
    inventory_item_id,
    display_name,
    original_file_name,
    description,
    mime_type,
    file_size_bytes,
    sha256_hex,
    storage_backend,
    storage_key,
    visibility,
    uploaded_by_user_id,
    created_at,
    updated_at
    "#
}

pub(super) async fn actor_for_user(
    pool: &PgPool,
    actor_user_id: UserId,
) -> Result<Actor, AttachmentError> {
    get_actor(pool, actor_user_id)
        .await
        .map_err(AttachmentError::UnexpectedError)?
        .ok_or_else(|| AttachmentError::Forbidden("Actor not found in the database".into()))
}

pub(super) fn validate_write_permission(
    actor: &Actor,
    laboratory_id: &LaboratoryId,
) -> Result<(), AttachmentError> {
    if actor.can_write_laboratory_resource(laboratory_id) {
        Ok(())
    } else {
        Err(AttachmentError::Forbidden(
            "You don't have permission to manage attachments for this laboratory".into(),
        ))
    }
}

pub(super) fn validate_read_permission(
    actor: &Actor,
    laboratory_id: &LaboratoryId,
) -> Result<bool, AttachmentError> {
    if actor.can_read_laboratory_resource(laboratory_id) {
        Ok(true)
    } else if actor.can_query_laboratory_resource(laboratory_id) {
        Ok(false)
    } else {
        Err(AttachmentError::Forbidden(
            "You do not have permission to view attachments for this laboratory".into(),
        ))
    }
}

pub(super) fn validate_attachment_read_permission(
    actor: &Actor,
    row: &AttachmentRow,
) -> Result<(), AttachmentError> {
    let laboratory_id = LaboratoryId::parse(row.laboratory_id)
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    let visibility = AttachmentVisibility::parse(&row.visibility)
        .map_err(|e| AttachmentError::UnexpectedError(anyhow!("{e}")))?;
    let can_read = match visibility {
        AttachmentVisibility::Public => actor.can_query_laboratory_resource(&laboratory_id),
        AttachmentVisibility::Internal => actor.can_read_laboratory_resource(&laboratory_id),
    };
    if can_read {
        Ok(())
    } else {
        Err(AttachmentError::Forbidden(
            "You do not have permission to view this attachment".into(),
        ))
    }
}

pub(super) async fn fetch_asset_target(
    pool: &PgPool,
    asset_id: Uuid,
) -> Result<Option<TargetRow>, AttachmentError> {
    sqlx::query_as::<_, TargetRow>(
        r#"
        SELECT laboratory_id
        FROM assets
        WHERE asset_id = $1
        "#,
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_asset_target_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    asset_id: Uuid,
) -> Result<Option<TargetRow>, AttachmentError> {
    sqlx::query_as::<_, TargetRow>(
        r#"
        SELECT laboratory_id
        FROM assets
        WHERE asset_id = $1
        FOR UPDATE
        "#,
    )
    .bind(asset_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_inventory_item_target(
    pool: &PgPool,
    inventory_item_id: Uuid,
) -> Result<Option<TargetRow>, AttachmentError> {
    sqlx::query_as::<_, TargetRow>(
        r#"
        SELECT laboratory_id
        FROM asset_inventory_items
        WHERE inventory_item_id = $1
        "#,
    )
    .bind(inventory_item_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_inventory_item_target_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
) -> Result<Option<TargetRow>, AttachmentError> {
    sqlx::query_as::<_, TargetRow>(
        r#"
        SELECT laboratory_id
        FROM asset_inventory_items
        WHERE inventory_item_id = $1
        FOR UPDATE
        "#,
    )
    .bind(inventory_item_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_attachments_for_target(
    pool: &PgPool,
    target: AttachmentTarget,
    include_internal: bool,
) -> Result<Vec<AttachmentRow>, AttachmentError> {
    let mut builder = QueryBuilder::<Postgres>::new("SELECT ");
    builder.push(attachment_columns());
    builder.push(" FROM attachments WHERE deleted_at IS NULL");
    match target {
        AttachmentTarget::Asset(asset_id) => {
            builder.push(" AND asset_id = ");
            builder.push_bind(asset_id);
        }
        AttachmentTarget::InventoryItem(inventory_item_id) => {
            builder.push(" AND inventory_item_id = ");
            builder.push_bind(inventory_item_id);
        }
    }
    if !include_internal {
        builder.push(" AND visibility = 'public'");
    }
    builder.push(" ORDER BY created_at DESC, attachment_id");
    builder
        .build_query_as::<AttachmentRow>()
        .fetch_all(pool)
        .await
        .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_laboratory_attachment_count(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    include_internal: bool,
) -> Result<i64, AttachmentError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*) FROM attachments WHERE deleted_at IS NULL AND laboratory_id = ",
    );
    builder.push_bind(*laboratory_id);
    if !include_internal {
        builder.push(" AND visibility = 'public'");
    }
    builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_laboratory_attachments(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    include_internal: bool,
    limit: i64,
    offset: i64,
) -> Result<Vec<AttachmentRow>, AttachmentError> {
    let mut builder = QueryBuilder::<Postgres>::new("SELECT ");
    builder.push(attachment_columns());
    builder.push(" FROM attachments WHERE deleted_at IS NULL AND laboratory_id = ");
    builder.push_bind(*laboratory_id);
    if !include_internal {
        builder.push(" AND visibility = 'public'");
    }
    builder.push(" ORDER BY created_at DESC, attachment_id LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);
    builder
        .build_query_as::<AttachmentRow>()
        .fetch_all(pool)
        .await
        .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_attachment(
    pool: &PgPool,
    attachment_id: Uuid,
) -> Result<Option<AttachmentRow>, AttachmentError> {
    sqlx::query_as::<_, AttachmentRow>(&format!(
        r#"
            SELECT {}
            FROM attachments
            WHERE attachment_id = $1
              AND deleted_at IS NULL
            "#,
        attachment_columns()
    ))
    .bind(attachment_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}

pub(super) async fn fetch_attachment_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    attachment_id: Uuid,
) -> Result<Option<AttachmentRow>, AttachmentError> {
    sqlx::query_as::<_, AttachmentRow>(&format!(
        r#"
            SELECT {}
            FROM attachments
            WHERE attachment_id = $1
              AND deleted_at IS NULL
            FOR UPDATE
            "#,
        attachment_columns()
    ))
    .bind(attachment_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| AttachmentError::UnexpectedError(e.into()))
}

pub(super) fn response_vec(rows: Vec<AttachmentRow>) -> Vec<AttachmentResponse> {
    rows.into_iter().map(AttachmentResponse::from).collect()
}

pub(super) fn attachment_audit_json(row: &AttachmentRow) -> serde_json::Value {
    json!({
        "attachment_id": row.attachment_id,
        "laboratory_id": row.laboratory_id,
        "asset_id": row.asset_id,
        "inventory_item_id": row.inventory_item_id,
        "display_name": row.display_name,
        "original_file_name": row.original_file_name,
        "description": row.description,
        "mime_type": row.mime_type,
        "file_size_bytes": row.file_size_bytes,
        "sha256_hex": row.sha256_hex,
        "visibility": row.visibility,
        "uploaded_by_user_id": row.uploaded_by_user_id,
    })
}

pub(super) fn map_database_error(error: sqlx::Error) -> AttachmentError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => {
                return AttachmentError::ConflictError("Attachment already exists".into());
            }
            Some("23503") => {
                return AttachmentError::ValidationError("Invalid referenced record".into());
            }
            Some("23514") => {
                return AttachmentError::ValidationError("Invalid attachment data".into());
            }
            _ => {}
        }
    }
    AttachmentError::UnexpectedError(error.into())
}
