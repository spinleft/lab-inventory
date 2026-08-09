use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{LaboratoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::ResponseError;
use actix_web::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum BorrowRequestError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for BorrowRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for BorrowRequestError {
    fn status_code(&self) -> StatusCode {
        match self {
            BorrowRequestError::ValidationError(_) => StatusCode::BAD_REQUEST,
            BorrowRequestError::Forbidden(_) => StatusCode::FORBIDDEN,
            BorrowRequestError::NotFound(_) => StatusCode::NOT_FOUND,
            BorrowRequestError::ConflictError(_) => StatusCode::CONFLICT,
            BorrowRequestError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

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

pub(crate) async fn actor_for_user(
    pool: &PgPool,
    actor_user_id: UserId,
) -> Result<Actor, BorrowRequestError> {
    get_actor(pool, actor_user_id)
        .await
        .map_err(BorrowRequestError::UnexpectedError)?
        .ok_or_else(|| BorrowRequestError::Forbidden("Actor not found in the database".into()))
}

pub(crate) fn validate_inventory_item_read_permission(
    actor: &Actor,
    laboratory_id: Uuid,
) -> Result<LaboratoryId, BorrowRequestError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id)
        .map_err(|e| BorrowRequestError::UnexpectedError(anyhow::anyhow!(e)))?;
    if actor.can_query_laboratory_resource(&laboratory_id) {
        Ok(laboratory_id)
    } else {
        Err(BorrowRequestError::Forbidden(
            "You do not have permission to view inventory items for this laboratory".into(),
        ))
    }
}

pub(crate) fn validate_request_actor(
    actor: &Actor,
    laboratory_id: LaboratoryId,
) -> Result<(), BorrowRequestError> {
    // Federated guests whose home lab matches the target lab
    if actor.is_guest() && actor.laboratory_id == Some(laboratory_id) {
        return Ok(());
    }
    // Same-server cross-laboratory admins and users can also request borrows;
    // they will be auto-registered as local guest links.
    if (actor.is_lab_admin() || actor.is_regular_user())
        && actor.laboratory_id.is_some()
        && actor.laboratory_id != Some(laboratory_id)
    {
        return Ok(());
    }
    Err(BorrowRequestError::Forbidden(
        "Only guest users or cross-laboratory users can request borrows".into(),
    ))
}

pub(crate) fn validate_resolver_actor(
    actor: &Actor,
    laboratory_id: LaboratoryId,
) -> Result<(), BorrowRequestError> {
    if (actor.is_lab_admin() || actor.is_regular_user())
        && actor.laboratory_id == Some(laboratory_id)
    {
        Ok(())
    } else {
        Err(BorrowRequestError::Forbidden(
            "Only this laboratory's administrators and users can approve or reject borrow requests"
                .into(),
        ))
    }
}

pub(crate) fn validate_borrow_request_status(
    status: Option<String>,
) -> Result<Option<String>, BorrowRequestError> {
    status
        .map(|status| {
            BorrowRequestStatus::parse(&status)
                .map(|status| status.as_str().to_string())
                .map_err(BorrowRequestError::ValidationError)
        })
        .transpose()
}

pub(crate) async fn fetch_guest_link_id(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: UserId,
    laboratory_id: LaboratoryId,
) -> Result<Uuid, BorrowRequestError> {
    // First, try to find an existing guest link for this user + laboratory.
    let guest_link_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT link_id
        FROM federation_guest_links
        WHERE local_guest_user_id = $1
          AND local_laboratory_id = $2
        "#,
    )
    .bind(*user_id)
    .bind(*laboratory_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| BorrowRequestError::UnexpectedError(e.into()))?;
    if let Some(link_id) = guest_link_id {
        return Ok(link_id);
    }

    // No existing link — if the actor is a same-server cross-lab user, auto-create
    // a local guest link so they can borrow just like a federated guest.
    let actor_row = sqlx::query!(
        r#"
        SELECT users.username, user_types.name AS user_type_name, users.laboratory_id
        FROM users
        JOIN user_types USING (user_type_id)
        WHERE users.user_id = $1
        "#,
        *user_id
    )
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| BorrowRequestError::UnexpectedError(e.into()))?
    .ok_or_else(|| BorrowRequestError::Forbidden("Actor not found in the database".into()))?;

    let is_cross_lab = matches!(actor_row.user_type_name.as_str(), "lab_admin" | "user")
        && actor_row.laboratory_id.is_some()
        && actor_row.laboratory_id != Some(*laboratory_id);

    if !is_cross_lab {
        return Err(BorrowRequestError::Forbidden(
            "Only federated guest users or cross-laboratory users can create borrow requests"
                .into(),
        ));
    }

    let home_laboratory_id = actor_row
        .laboratory_id
        .ok_or_else(|| BorrowRequestError::Forbidden("Actor has no home laboratory".into()))?;
    let link_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO federation_guest_links (
            link_id,
            local_laboratory_id,
            remote_node_id,
            remote_laboratory_id,
            remote_user_id,
            remote_username,
            remote_user_type,
            local_guest_user_id,
            source
        )
        VALUES ($1, $2, NULL, $3, $4, $5, $6, $4, 'local')
        "#,
    )
    .bind(link_id)
    .bind(*laboratory_id)
    .bind(home_laboratory_id)
    .bind(*user_id)
    .bind(&actor_row.username)
    .bind(&actor_row.user_type_name)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| BorrowRequestError::UnexpectedError(e.into()))?;

    Ok(link_id)
}

pub(crate) async fn fetch_user_snapshot(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: UserId,
) -> Result<(String, String), BorrowRequestError> {
    let row: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT users.username, user_types.name
        FROM users
        JOIN user_types USING (user_type_id)
        WHERE users.user_id = $1
        "#,
    )
    .bind(*user_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|e| BorrowRequestError::UnexpectedError(e.into()))?;
    row.ok_or_else(|| BorrowRequestError::Forbidden("Actor not found in the database".into()))
}

pub(crate) fn borrow_request_inventory_select() -> &'static str {
    r#"
    SELECT
        requests.borrow_request_id,
        requests.local_laboratory_id,
        requests.inventory_item_id,
        requests.requester_user_id,
        requests.requester_username,
        requests.requester_user_type,
        requests.requester_guest_link_id,
        requests.request_note,
        requests.status,
        requests.reviewed_by_user_id,
        requests.reviewed_by_username,
        requests.reviewed_by_user_type,
        requests.reviewed_at,
        requests.decision_note,
        requests.created_at,
        requests.updated_at,
        asset_inventory_items.status AS inventory_status,
        asset_inventory_items.serial_number AS inventory_serial_number,
        asset_inventory_items.batch_number AS inventory_batch_number,
        assets.name AS asset_name,
        assets.model AS asset_model
    FROM asset_inventory_items
    JOIN assets
      ON assets.asset_id = asset_inventory_items.asset_id
    "#
}

pub(crate) async fn fetch_borrow_request_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    local_laboratory_id: Uuid,
    borrow_request_id: Uuid,
) -> Result<Option<BorrowRequestRow>, BorrowRequestError> {
    let query = format!(
        "{} INNER JOIN federation_borrow_requests AS requests ON requests.inventory_item_id = asset_inventory_items.inventory_item_id WHERE requests.local_laboratory_id = $1 AND requests.borrow_request_id = $2 FOR UPDATE OF requests, asset_inventory_items",
        borrow_request_inventory_select()
    );
    sqlx::query_as::<_, BorrowRequestRow>(&query)
        .bind(local_laboratory_id)
        .bind(borrow_request_id)
        .fetch_optional(transaction.as_mut())
        .await
        .map_err(|e| BorrowRequestError::UnexpectedError(e.into()))
}

pub(crate) fn borrow_request_title(row: &BorrowRequestRow) -> String {
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

pub(crate) fn borrow_request_audit_details(
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

pub(crate) async fn record_borrow_request_audit(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    action: AuditAction,
    borrow_request_id: Uuid,
    inventory_item_id: Uuid,
    status: &str,
    decision_note: Option<&str>,
) -> Result<(), BorrowRequestError> {
    record_audit(
        transaction,
        actor,
        action,
        AuditResource::BorrowRequest,
        Some(borrow_request_id),
        borrow_request_audit_details(borrow_request_id, inventory_item_id, status, decision_note),
    )
    .await
    .map_err(BorrowRequestError::UnexpectedError)
}
