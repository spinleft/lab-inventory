use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct Laboratory {
    pub(super) laboratory_id: Uuid,
    pub(super) name: String,
    pub(super) address: String,
    pub(super) description: Option<String>,
    pub(super) contact: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}
