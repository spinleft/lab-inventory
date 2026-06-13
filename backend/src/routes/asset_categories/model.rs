use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub struct AssetCategory {
    pub category_id: Uuid,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub parent_category_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub level: i32,
    pub path_name: String,
    pub path: Value,
    pub children_count: i64,
    pub asset_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
