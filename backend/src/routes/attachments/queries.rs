//! Every SQL statement the attachment routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - functions never return a handler error type, only
//!   [`AttachmentDatabaseError`], so any handler can reuse them
use super::model::{AttachmentFileRow, AttachmentRow, AttachmentTarget};
use crate::domain::{AssetId, AttachmentId, InventoryItemId, LaboratoryId, UserId};
use crate::routes::file_uploads::ConsumedFileUpload;
use crate::utils::error_chain_fmt;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(crate) enum AttachmentDatabaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for AttachmentDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> AttachmentDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => {
                return AttachmentDatabaseError::Conflict("Attachment already exists".into());
            }
            Some("23503") => {
                return AttachmentDatabaseError::Validation("Invalid referenced record".into());
            }
            Some("23514") => {
                return AttachmentDatabaseError::Validation("Invalid attachment data".into());
            }
            _ => {}
        }
    }

    AttachmentDatabaseError::Unexpected(error.into())
}

// ---------------------------------------------------------------------------
// single attachments
// ---------------------------------------------------------------------------

pub(super) async fn fetch_attachment(
    pool: &PgPool,
    attachment_id: AttachmentId,
) -> Result<Option<AttachmentRow>, anyhow::Error> {
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
        WHERE assignments.attachment_id = $1
        "#,
        Uuid::from(attachment_id)
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch attachment")
}

/// Same projection as [`fetch_attachment`], but takes the row lock the write
/// paths need. `query_as!` requires a literal, so the column list cannot be
/// shared.
pub(super) async fn fetch_attachment_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    attachment_id: AttachmentId,
) -> Result<Option<AttachmentRow>, anyhow::Error> {
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
        WHERE assignments.attachment_id = $1
        FOR UPDATE
        "#,
        Uuid::from(attachment_id)
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch attachment for update")
}

/// Just the columns a download needs, so streaming a file does not pull the
/// whole assignment row along with it.
pub(super) async fn fetch_attachment_file(
    pool: &PgPool,
    attachment_id: AttachmentId,
) -> Result<Option<AttachmentFileRow>, anyhow::Error> {
    sqlx::query_as!(
        AttachmentFileRow,
        r#"
        SELECT storage_key, original_file_name, mime_type
        FROM asset_attachment_assignments AS assignments
        JOIN files ON files.file_id = assignments.file_id
        WHERE assignments.attachment_id = $1
        "#,
        Uuid::from(attachment_id)
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch attachment file")
}

// ---------------------------------------------------------------------------
// listings
// ---------------------------------------------------------------------------

pub(super) async fn fetch_asset_attachments(
    pool: &PgPool,
    asset_id: AssetId,
    include_internal: bool,
) -> Result<Vec<AttachmentRow>, anyhow::Error> {
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
    .context("Failed to fetch asset attachments")
}

pub(super) async fn fetch_inventory_item_attachments(
    pool: &PgPool,
    inventory_item_id: InventoryItemId,
    include_internal: bool,
) -> Result<Vec<AttachmentRow>, anyhow::Error> {
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
    .context("Failed to fetch inventory item attachments")
}

pub(super) async fn count_laboratory_attachments(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    include_internal: bool,
) -> Result<i64, anyhow::Error> {
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
    .context("Failed to count laboratory attachments")
}

pub(super) async fn fetch_laboratory_attachments(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
    include_internal: bool,
    limit: i64,
    offset: i64,
) -> Result<Vec<AttachmentRow>, anyhow::Error> {
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
    .context("Failed to fetch laboratory attachments")
}

// ---------------------------------------------------------------------------
// writes
// ---------------------------------------------------------------------------

/// Moves a consumed upload into permanent storage: one `files` row and one
/// assignment pointing at it, written together so an attachment can never end up
/// without its file.
pub(super) async fn insert_attachment(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: UserId,
    target: &AttachmentTarget,
    upload: &ConsumedFileUpload,
    display_name: &str,
    description: Option<&str>,
    is_public: bool,
) -> Result<AttachmentRow, AttachmentDatabaseError> {
    let (asset_id, inventory_item_id) = match target {
        AttachmentTarget::Asset(asset_id) => (Some(*asset_id), None),
        AttachmentTarget::InventoryItem(inventory_item_id) => (None, Some(*inventory_item_id)),
    };

    sqlx::query_as::<_, AttachmentRow>(
        r#"
        WITH inserted_file AS (
            INSERT INTO files (
                file_id,
                laboratory_id,
                storage_backend,
                storage_key,
                original_file_name,
                mime_type,
                file_size_bytes,
                sha256_hex,
                uploaded_by_user_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING
                file_id,
                laboratory_id,
                storage_backend,
                storage_key,
                original_file_name,
                mime_type,
                file_size_bytes,
                sha256_hex,
                uploaded_by_user_id,
                created_at
        ),
        inserted_assignment AS (
            INSERT INTO asset_attachment_assignments (
                attachment_id,
                laboratory_id,
                file_id,
                asset_id,
                inventory_item_id,
                display_name,
                description,
                is_public,
                assigned_by_user_id
            )
            SELECT
                $10,
                inserted_file.laboratory_id,
                inserted_file.file_id,
                $11,
                $12,
                $13,
                $14,
                $15,
                $16
            FROM inserted_file
            RETURNING
                attachment_id,
                laboratory_id,
                file_id,
                asset_id,
                inventory_item_id,
                display_name,
                description,
                is_public,
                assigned_by_user_id,
                created_at,
                updated_at
        )
        SELECT
            inserted_assignment.attachment_id,
            inserted_assignment.laboratory_id,
            inserted_assignment.file_id,
            inserted_assignment.asset_id,
            inserted_assignment.inventory_item_id,
            inserted_assignment.display_name,
            inserted_assignment.description,
            inserted_assignment.is_public,
            inserted_assignment.assigned_by_user_id,
            inserted_assignment.created_at,
            inserted_assignment.updated_at,
            inserted_file.storage_backend,
            inserted_file.storage_key,
            inserted_file.original_file_name,
            inserted_file.mime_type,
            inserted_file.file_size_bytes,
            inserted_file.sha256_hex,
            inserted_file.uploaded_by_user_id,
            inserted_file.created_at AS file_created_at
        FROM inserted_assignment
        JOIN inserted_file
          ON inserted_file.file_id = inserted_assignment.file_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(upload.laboratory_id)
    .bind(&upload.storage_backend)
    .bind(&upload.storage_key)
    .bind(&upload.original_file_name)
    .bind(upload.mime_type.as_deref())
    .bind(upload.file_size_bytes)
    .bind(&upload.sha256_hex)
    .bind(upload.uploaded_by_user_id)
    .bind(Uuid::new_v4())
    .bind(asset_id)
    .bind(inventory_item_id)
    .bind(display_name)
    .bind(description)
    .bind(is_public)
    .bind(*actor_user_id)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

#[tracing::instrument(
    name = "Updating attachment in the database",
    skip(transaction, display_name, description, is_public),
    fields(attachment_id=%attachment_id)
)]
pub(super) async fn update_attachment_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    attachment_id: AttachmentId,
    display_name: &str,
    description: Option<&str>,
    is_public: bool,
) -> Result<AttachmentRow, AttachmentDatabaseError> {
    sqlx::query_as!(
        AttachmentRow,
        r#"
        WITH updated_assignment AS (
            UPDATE asset_attachment_assignments
            SET
                display_name = $2,
                description = $3,
                is_public = $4,
                updated_at = now()
            WHERE attachment_id = $1
            RETURNING *
        )
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
        FROM updated_assignment AS assignments
        JOIN files ON files.file_id = assignments.file_id
        "#,
        Uuid::from(attachment_id),
        display_name,
        description,
        is_public,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

/// Drops the assignment and the file behind it in one statement. The stored blob
/// itself is removed by the caller once the transaction has committed.
#[tracing::instrument(
    name = "Deleting attachment from the database",
    skip(transaction),
    fields(attachment_id=%attachment_id)
)]
pub(super) async fn delete_attachment_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    attachment_id: AttachmentId,
) -> Result<(), AttachmentDatabaseError> {
    sqlx::query!(
        r#"
        WITH deleted_assignment AS (
            DELETE FROM asset_attachment_assignments
            WHERE attachment_id = $1
            RETURNING file_id
        )
        DELETE FROM files
        WHERE file_id IN (SELECT file_id FROM deleted_assignment)
        "#,
        Uuid::from(attachment_id)
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// owning records
// ---------------------------------------------------------------------------

pub(super) async fn fetch_asset_laboratory_id(
    pool: &PgPool,
    asset_id: AssetId,
) -> Result<Option<LaboratoryId>, anyhow::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT laboratory_id
        FROM assets
        WHERE asset_id = $1
        "#,
        Uuid::from(asset_id)
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch asset laboratory")
    .map(|laboratory_id| laboratory_id.map(Into::into))
}

pub(super) async fn fetch_inventory_item_laboratory_id(
    pool: &PgPool,
    inventory_item_id: InventoryItemId,
) -> Result<Option<LaboratoryId>, anyhow::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT laboratory_id
        FROM asset_inventory_items
        WHERE inventory_item_id = $1
        "#,
        Uuid::from(inventory_item_id)
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch inventory item laboratory")
    .map(|laboratory_id| laboratory_id.map(Into::into))
}
