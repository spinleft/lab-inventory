use crate::access_control::Actor;
use crate::domain::UserId;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub trait AuditActor {
    fn audit_user_id(&self) -> UserId;
}

impl AuditActor for UserId {
    fn audit_user_id(&self) -> UserId {
        *self
    }
}

impl AuditActor for &UserId {
    fn audit_user_id(&self) -> UserId {
        **self
    }
}

impl AuditActor for &Actor {
    fn audit_user_id(&self) -> UserId {
        self.user_id
    }
}

pub enum AuditAction {
    Create,
    Update,
    Delete,
    Adjust,
    Move,
    Stocktake,
    Allocate,
    ReleaseAllocation,
    Print,
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
            Self::Print => "print",
        }
    }
}

pub enum AuditResource {
    Laboratory,
    User,
    GuestRegistrationCode,
    AssetCategory,
    Location,
    Asset,
    AssetParameter,
    InventoryItem,
    BorrowRequest,
    Attachment,
    Unit,
    FederationTrust,
    FederationGuestLink,
    LabelPrinter,
}

impl AuditResource {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Laboratory => "laboratory",
            Self::User => "user",
            Self::GuestRegistrationCode => "guest_registration_code",
            Self::AssetCategory => "asset_category",
            Self::Location => "location",
            Self::Asset => "asset",
            Self::AssetParameter => "asset_parameter",
            Self::InventoryItem => "inventory_item",
            Self::BorrowRequest => "borrow_request",
            Self::Attachment => "attachment",
            Self::Unit => "unit",
            Self::FederationTrust => "federation_trust",
            Self::FederationGuestLink => "federation_guest_link",
            Self::LabelPrinter => "label_printer",
        }
    }
}

pub async fn record_audit(
    transaction: &mut Transaction<'_, Postgres>,
    actor: impl AuditActor,
    action: AuditAction,
    resource_type: AuditResource,
    resource_id: Option<Uuid>,
    details: Value,
) -> Result<(), anyhow::Error> {
    let actor_user_id = actor.audit_user_id();
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
    .bind(*actor_user_id)
    .bind(action.as_str())
    .bind(resource_type.as_str())
    .bind(resource_id)
    .bind(details)
    .execute(transaction.as_mut())
    .await?;

    Ok(())
}
