use crate::authentication::Actor;
use crate::utils::ApiError;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub enum AuditAction {
    Create,
    Update,
    Delete,
    Adjust,
    Move,
    Stocktake,
    Allocate,
    ReleaseAllocation,
    Approve,
    Reject,
    Cancel,
    BorrowOut,
    Return,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Adjust => "adjust",
            Self::Move => "move",
            Self::Stocktake => "stocktake",
            Self::Allocate => "allocate",
            Self::ReleaseAllocation => "release_allocation",
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::Cancel => "cancel",
            Self::BorrowOut => "borrow_out",
            Self::Return => "return",
        }
    }
}

pub enum AuditResource {
    Laboratory,
    User,
    AssetCategory,
    Location,
    Asset,
    InventoryItem,
    BorrowRequest,
    MaintenanceRecord,
    MaintenanceSchedule,
    Attachment,
}

impl AuditResource {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Laboratory => "laboratory",
            Self::User => "user",
            Self::AssetCategory => "asset_category",
            Self::Location => "location",
            Self::Asset => "asset",
            Self::InventoryItem => "inventory_item",
            Self::BorrowRequest => "borrow_request",
            Self::MaintenanceRecord => "maintenance_record",
            Self::MaintenanceSchedule => "maintenance_schedule",
            Self::Attachment => "attachment",
        }
    }
}

pub async fn record_audit(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    target_laboratory_id: Option<Uuid>,
    action: AuditAction,
    resource_type: AuditResource,
    resource_id: Option<Uuid>,
    details: Value,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs (
            audit_log_id,
            actor_user_id,
            actor_laboratory_id,
            target_laboratory_id,
            action,
            resource_type,
            resource_id,
            details
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(actor.user_id)
    .bind(actor.laboratory_id)
    .bind(target_laboratory_id)
    .bind(action.as_str())
    .bind(resource_type.as_str())
    .bind(resource_id)
    .bind(details)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(())
}
