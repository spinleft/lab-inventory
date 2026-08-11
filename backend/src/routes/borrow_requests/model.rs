use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum BorrowRequestStatus {
    Pending,
    Approved,
    Rejected,
}

impl BorrowRequestStatus {
    pub(super) fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            _ => Err(format!("{value} is not a valid borrow request status.")),
        }
    }

    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Clone, sqlx::FromRow)]
pub(super) struct BorrowRequestRow {
    pub(super) borrow_request_id: Uuid,
    pub(super) local_laboratory_id: Uuid,
    pub(super) inventory_item_id: Uuid,
    pub(super) requester_user_id: Option<Uuid>,
    pub(super) requester_username: String,
    pub(super) requester_user_type: String,
    pub(super) requester_guest_link_id: Option<Uuid>,
    pub(super) request_note: Option<String>,
    pub(super) status: String,
    pub(super) reviewed_by_user_id: Option<Uuid>,
    pub(super) reviewed_by_username: Option<String>,
    pub(super) reviewed_by_user_type: Option<String>,
    pub(super) reviewed_at: Option<DateTime<Utc>>,
    pub(super) decision_note: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) inventory_status: String,
    pub(super) inventory_serial_number: Option<String>,
    pub(super) inventory_batch_number: Option<String>,
    pub(super) asset_name: String,
    pub(super) asset_model: Option<String>,
}

/// Who a user is as far as borrowing is concerned: the name and role copied onto
/// a request.
#[derive(sqlx::FromRow)]
pub(super) struct BorrowActorRow {
    pub(super) username: String,
    pub(super) user_type_name: String,
}

#[derive(Serialize)]
pub(super) struct BorrowRequestResponse {
    borrow_request_id: Uuid,
    local_laboratory_id: Uuid,
    inventory_item_id: Uuid,
    inventory_item_title: String,
    inventory_status: String,
    requester_user_id: Option<Uuid>,
    requester_username: String,
    requester_user_type: String,
    requester_guest_link_id: Option<Uuid>,
    request_note: Option<String>,
    status: String,
    reviewed_by_user_id: Option<Uuid>,
    reviewed_by_username: Option<String>,
    reviewed_by_user_type: Option<String>,
    reviewed_at: Option<DateTime<Utc>>,
    decision_note: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    asset_name: String,
    asset_model: Option<String>,
}

impl From<BorrowRequestRow> for BorrowRequestResponse {
    fn from(row: BorrowRequestRow) -> Self {
        Self {
            borrow_request_id: row.borrow_request_id,
            local_laboratory_id: row.local_laboratory_id,
            inventory_item_id: row.inventory_item_id,
            inventory_item_title: borrow_request_title(&row),
            inventory_status: row.inventory_status,
            requester_user_id: row.requester_user_id,
            requester_username: row.requester_username,
            requester_user_type: row.requester_user_type,
            requester_guest_link_id: row.requester_guest_link_id,
            request_note: row.request_note,
            status: row.status,
            reviewed_by_user_id: row.reviewed_by_user_id,
            reviewed_by_username: row.reviewed_by_username,
            reviewed_by_user_type: row.reviewed_by_user_type,
            reviewed_at: row.reviewed_at,
            decision_note: row.decision_note,
            created_at: row.created_at,
            updated_at: row.updated_at,
            asset_name: row.asset_name,
            asset_model: row.asset_model,
        }
    }
}

/// How the borrowed thing is named in a list: whatever identifies the individual
/// item, falling back to the name of the asset it is one of.
fn borrow_request_title(row: &BorrowRequestRow) -> String {
    row.inventory_serial_number
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            row.inventory_batch_number
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| row.asset_name.clone())
}

pub(super) fn borrow_request_audit_details(
    borrow_request_id: Uuid,
    inventory_item_id: Uuid,
    status: &str,
    decision_note: Option<&str>,
) -> serde_json::Value {
    json!({
        "borrow_request_id": borrow_request_id,
        "inventory_item_id": inventory_item_id,
        "status": status,
        "decision_note": decision_note,
    })
}
