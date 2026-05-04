use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub struct Unit {
    pub unit_id: Uuid,
    pub code: String,
    pub name: String,
    pub symbol: String,
    pub dimension: String,
    pub scale_to_base: f64,
    pub allow_decimal: bool,
    pub created_at: DateTime<Utc>,
}
