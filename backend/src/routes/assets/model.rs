use crate::authentication::Actor;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct AssetRow {
    pub asset_id: Uuid,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub asset_kind: String,
    pub tracking_mode: String,
    pub name: String,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub default_unit_id: Uuid,
    pub default_unit_code: String,
    pub minimum_stock_quantity: Option<f64>,
    pub minimum_stock_unit_id: Option<Uuid>,
    pub minimum_stock_unit_code: Option<String>,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct AssetResponse {
    pub asset_id: Uuid,
    pub laboratory_id: Uuid,
    pub laboratory_name: String,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub asset_kind: String,
    pub tracking_mode: String,
    pub name: String,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub default_unit_id: Uuid,
    pub default_unit_code: String,
    pub minimum_stock_quantity: Option<f64>,
    pub minimum_stock_unit_id: Option<Uuid>,
    pub minimum_stock_unit_code: Option<String>,
    pub public_notes: Option<String>,
    pub internal_notes: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AssetResponse {
    pub fn from_row(row: AssetRow, actor: &Actor) -> Self {
        let show_sensitive = actor.is_owner() || actor.is_same_laboratory(row.laboratory_id);
        Self {
            asset_id: row.asset_id,
            laboratory_id: row.laboratory_id,
            laboratory_name: row.laboratory_name,
            category_id: row.category_id,
            category_name: row.category_name,
            asset_kind: row.asset_kind,
            tracking_mode: row.tracking_mode,
            name: row.name,
            model: row.model,
            manufacturer: row.manufacturer,
            default_unit_id: row.default_unit_id,
            default_unit_code: row.default_unit_code,
            minimum_stock_quantity: row.minimum_stock_quantity,
            minimum_stock_unit_id: row.minimum_stock_unit_id,
            minimum_stock_unit_code: row.minimum_stock_unit_code,
            public_notes: row.public_notes,
            internal_notes: show_sensitive.then_some(row.internal_notes).flatten(),
            is_archived: row.is_archived,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
