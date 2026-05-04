use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct Attachment {
    pub attachment_id: Uuid,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub resource_type: String,
    pub resource_id: Uuid,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: i64,
    pub storage_url: String,
    pub visibility: String,
    pub uploaded_by_user_id: Option<Uuid>,
    pub uploaded_by_username: Option<String>,
    pub created_at: DateTime<Utc>,
}
