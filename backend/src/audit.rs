use crate::access_control::Actor;
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
        }
    }
}

pub enum AuditResource {
    Laboratory,
    User,
    AssetCategory,
    Location,
    Asset,
    AssetParameter,
    InventoryItem,
    Attachment,
    Unit,
}

impl AuditResource {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Laboratory => "laboratory",
            Self::User => "user",
            Self::AssetCategory => "asset_category",
            Self::Location => "location",
            Self::Asset => "asset",
            Self::AssetParameter => "asset_parameter",
            Self::InventoryItem => "inventory_item",
            Self::Attachment => "attachment",
            Self::Unit => "unit",
        }
    }
}

pub async fn record_audit(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    action: AuditAction,
    resource_type: AuditResource,
    resource_id: Option<Uuid>,
    details: Value,
) -> Result<(), anyhow::Error> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs (
            audit_log_id,
            actor_user_id,
            action,
            resource_type,
            resource_id,
            details
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(*actor.user_id)
    .bind(action.as_str())
    .bind(resource_type.as_str())
    .bind(resource_id)
    .bind(details)
    .execute(transaction.as_mut())
    .await?;

    Ok(())
}
