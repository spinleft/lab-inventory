use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub struct Location {
    pub location_id: Uuid,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub parent_location_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
