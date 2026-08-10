use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct AttachmentRow {
    pub attachment_id: Uuid,
    pub laboratory_id: Uuid,
    pub file_id: Uuid,
    pub asset_id: Option<Uuid>,
    pub inventory_item_id: Option<Uuid>,
    pub display_name: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub assigned_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub storage_backend: String,
    pub storage_key: String,
    pub original_file_name: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: i64,
    pub sha256_hex: String,
    pub uploaded_by_user_id: Option<Uuid>,
    pub file_created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub(super) struct AttachmentResponse {
    attachment_id: Uuid,
    laboratory_id: Uuid,
    file_id: Uuid,
    target: AttachmentTargetResponse,
    display_name: String,
    description: Option<String>,
    is_public: bool,
    assigned_by_user_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    file: AttachmentFileResponse,
}

impl From<AttachmentRow> for AttachmentResponse {
    fn from(row: AttachmentRow) -> Self {
        let target = match (row.asset_id, row.inventory_item_id) {
            (Some(asset_id), None) => AttachmentTargetResponse::Asset { id: asset_id },
            (None, Some(inventory_item_id)) => AttachmentTargetResponse::InventoryItem {
                id: inventory_item_id,
            },
            _ => unreachable!("attachment assignment must have exactly one target"),
        };
        let file = AttachmentFileResponse {
            file_id: row.file_id,
            original_file_name: row.original_file_name,
            mime_type: row.mime_type,
            file_size_bytes: row.file_size_bytes,
            sha256_hex: row.sha256_hex,
            uploaded_by_user_id: row.uploaded_by_user_id,
            created_at: row.file_created_at,
        };

        Self {
            attachment_id: row.attachment_id,
            laboratory_id: row.laboratory_id,
            file_id: row.file_id,
            target,
            display_name: row.display_name,
            description: row.description,
            is_public: row.is_public,
            assigned_by_user_id: row.assigned_by_user_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            file,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum AttachmentTargetResponse {
    Asset { id: Uuid },
    InventoryItem { id: Uuid },
}

#[derive(Serialize)]
pub(super) struct AttachmentFileResponse {
    file_id: Uuid,
    original_file_name: String,
    mime_type: Option<String>,
    file_size_bytes: i64,
    sha256_hex: String,
    uploaded_by_user_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

pub(crate) enum AttachmentTarget {
    Asset(Uuid),
    InventoryItem(Uuid),
}

/// Just the columns a download needs to stream the stored blob back.
#[derive(sqlx::FromRow)]
pub(super) struct AttachmentFileRow {
    pub(super) storage_key: String,
    pub(super) original_file_name: String,
    pub(super) mime_type: Option<String>,
}

pub(super) fn create_attachment_rollback_details(row: &AttachmentRow) -> serde_json::Value {
    json!({
        "rollback": {
            "operation": "delete",
            "resource_type": "attachment",
            "where": {
                "attachment_id": row.attachment_id,
                "file_id": row.file_id,
                "storage_key": &row.storage_key,
            },
        },
    })
}

pub(super) fn delete_attachment_rollback_details(row: &AttachmentRow) -> serde_json::Value {
    json!({
        "rollback": {
            "operation": "create",
            "resource_type": "attachment",
            "values": {
                "attachment_id": row.attachment_id,
                "laboratory_id": row.laboratory_id,
                "file_id": row.file_id,
                "asset_id": row.asset_id,
                "inventory_item_id": row.inventory_item_id,
                "display_name": &row.display_name,
                "description": row.description.as_deref(),
                "is_public": row.is_public,
                "assigned_by_user_id": row.assigned_by_user_id,
                "created_at": row.created_at,
                "updated_at": row.updated_at,
                "file": {
                    "file_id": row.file_id,
                    "laboratory_id": row.laboratory_id,
                    "storage_backend": &row.storage_backend,
                    "storage_key": &row.storage_key,
                    "original_file_name": &row.original_file_name,
                    "mime_type": row.mime_type.as_deref(),
                    "file_size_bytes": row.file_size_bytes,
                    "sha256_hex": &row.sha256_hex,
                    "uploaded_by_user_id": row.uploaded_by_user_id,
                    "created_at": row.file_created_at,
                },
            },
        },
    })
}

pub(super) fn update_attachment_rollback_details(row: &AttachmentRow) -> serde_json::Value {
    json!({
        "rollback": {
            "operation": "update",
            "resource_type": "attachment",
            "where": {
                "attachment_id": row.attachment_id,
            },
            "values": {
                "display_name": &row.display_name,
                "description": row.description.as_deref(),
                "is_public": row.is_public,
                "updated_at": row.updated_at,
            },
        },
    })
}
