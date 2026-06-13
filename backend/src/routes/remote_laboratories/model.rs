use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub struct RemoteLaboratory {
    pub remote_laboratory_id: Uuid,
    pub name: String,
    pub api_base_url: String,
    pub is_enabled: bool,
    pub key_id: String,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub struct RemoteLaboratorySecret {
    pub api_base_url: String,
    pub is_enabled: bool,
    pub key_id: String,
    pub shared_secret: String,
}
