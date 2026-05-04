use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct AuditLog {
    pub audit_log_id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub actor_username: Option<String>,
    pub actor_laboratory_id: Option<Uuid>,
    pub actor_laboratory_name: Option<String>,
    pub target_laboratory_id: Option<Uuid>,
    pub target_laboratory_name: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub details: Value,
    pub created_at: DateTime<Utc>,
}
