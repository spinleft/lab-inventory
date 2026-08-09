use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct FileUploadResponse {
    upload_id: Uuid,
    laboratory_id: Uuid,
    original_file_name: String,
    mime_type: Option<String>,
    file_size_bytes: i64,
    sha256_hex: String,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

#[derive(Clone, sqlx::FromRow)]
pub(super) struct FileUploadRow {
    pub(super) upload_id: Uuid,
    pub(super) laboratory_id: Uuid,
    pub(super) storage_backend: String,
    pub(super) storage_key: String,
    pub(super) original_file_name: String,
    pub(super) mime_type: Option<String>,
    pub(super) file_size_bytes: i64,
    pub(super) sha256_hex: String,
    pub(super) uploaded_by_user_id: Uuid,
    pub(super) expires_at: DateTime<Utc>,
    pub(super) consumed_at: Option<DateTime<Utc>>,
}

pub struct ConsumedFileUpload {
    pub laboratory_id: Uuid,
    pub storage_backend: String,
    pub storage_key: String,
    pub original_file_name: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: i64,
    pub sha256_hex: String,
    pub uploaded_by_user_id: Uuid,
}

impl From<FileUploadRow> for ConsumedFileUpload {
    fn from(row: FileUploadRow) -> Self {
        Self {
            laboratory_id: row.laboratory_id,
            storage_backend: row.storage_backend,
            storage_key: row.storage_key,
            original_file_name: row.original_file_name,
            mime_type: row.mime_type,
            file_size_bytes: row.file_size_bytes,
            sha256_hex: row.sha256_hex,
            uploaded_by_user_id: row.uploaded_by_user_id,
        }
    }
}
