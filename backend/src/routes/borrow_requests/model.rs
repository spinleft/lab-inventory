use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

pub const PENDING: &str = "pending";
pub const APPROVED: &str = "approved";
pub const BORROWED: &str = "borrowed";

#[derive(Serialize, sqlx::FromRow)]
pub struct BorrowRequest {
    pub borrow_request_id: Uuid,
    pub inventory_item_id: Uuid,
    pub asset_id: Uuid,
    pub asset_name: String,
    pub asset_model: Option<String>,
    pub requester_user_id: Uuid,
    pub requester_username: String,
    pub requester_laboratory_id: Uuid,
    pub requester_laboratory_name: String,
    pub owner_laboratory_id: Uuid,
    pub owner_laboratory_name: String,
    pub requested_quantity: f64,
    pub unit_id: Uuid,
    pub unit_code: String,
    pub expected_borrowed_at: Option<DateTime<Utc>>,
    pub expected_returned_at: Option<DateTime<Utc>>,
    pub purpose: String,
    pub status: String,
    pub reviewed_by_user_id: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_comment: Option<String>,
    pub borrowed_by_user_id: Option<Uuid>,
    pub borrowed_at: Option<DateTime<Utc>>,
    pub returned_by_user_id: Option<Uuid>,
    pub returned_at: Option<DateTime<Utc>>,
    pub cancelled_by_user_id: Option<Uuid>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub(super) struct BorrowRequestForUpdate {
    pub borrow_request_id: Uuid,
    pub inventory_item_id: Uuid,
    pub requester_laboratory_id: Uuid,
    pub owner_laboratory_id: Uuid,
    pub requested_quantity: f64,
    pub status: String,
}

#[derive(sqlx::FromRow)]
pub(super) struct InventoryItemForBorrow {
    pub inventory_item_id: Uuid,
    pub laboratory_id: Uuid,
    pub tracking_mode: String,
    pub quantity_on_hand: f64,
    pub quantity_allocated: f64,
    pub unit_id: Uuid,
    pub unit_allow_decimal: bool,
    pub is_cross_lab_borrowable: bool,
    pub status: String,
    pub location_id: Option<Uuid>,
}
