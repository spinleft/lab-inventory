use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

/// The error every federation route answers with.
///
/// Federation spans two audiences — an administrator driving pairing from the
/// UI and another server speaking the signed protocol — so one type carries both
/// the ordinary outcomes and the protocol-level ones (`Unauthorized` for a
/// failed signature, `BadGateway` for a remote node that misbehaved).
#[derive(thiserror::Error)]
pub enum FederationError {
    #[error("Federation is disabled")]
    Disabled,
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error("{0}")]
    BadGateway(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for FederationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for FederationError {
    fn status_code(&self) -> StatusCode {
        match self {
            FederationError::Disabled => StatusCode::FORBIDDEN,
            FederationError::ValidationError(_) => StatusCode::BAD_REQUEST,
            FederationError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            FederationError::Forbidden(_) => StatusCode::FORBIDDEN,
            FederationError::NotFound(_) => StatusCode::NOT_FOUND,
            FederationError::ConflictError(_) => StatusCode::CONFLICT,
            FederationError::BadGateway(_) => StatusCode::BAD_GATEWAY,
            FederationError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(json!({ "error": self.to_string() }))
    }
}

#[derive(Clone, sqlx::FromRow)]
pub(super) struct LocalNodeRow {
    pub(super) node_id: Uuid,
    pub(super) public_base_url: String,
}

#[derive(Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub(super) struct RemoteNodeRow {
    pub(super) remote_node_id: Uuid,
    pub(super) base_url: String,
    pub(super) display_name: Option<String>,
    pub(super) shared_secret: String,
    pub(super) shared_secret_hash: String,
    pub(super) tls_certificate_sha256: Option<String>,
    pub(super) status: String,
    pub(super) key_version: i32,
    pub(super) last_handshake_at: Option<DateTime<Utc>>,
}

#[derive(Clone, sqlx::FromRow)]
pub(super) struct TrustRow {
    pub(super) trust_id: Uuid,
    pub(super) local_laboratory_id: Uuid,
    pub(super) remote_node_id: Uuid,
    pub(super) remote_laboratory_id: Uuid,
    pub(super) remote_laboratory_name: Option<String>,
    pub(super) status: String,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) revoked_at: Option<DateTime<Utc>>,
}

/// A trust listed for display, with the remote node's address joined in so the
/// row can be serialized straight to the client.
#[derive(Serialize, sqlx::FromRow)]
pub(super) struct TrustWithRemoteRow {
    trust_id: Uuid,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_base_url: String,
    remote_laboratory_id: Uuid,
    remote_laboratory_name: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub(super) struct TrustResponse {
    trust_id: Uuid,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_base_url: String,
    remote_laboratory_id: Uuid,
    remote_laboratory_name: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

impl TrustResponse {
    pub(super) fn from_parts(trust: TrustRow, remote: RemoteNodeRow) -> Self {
        Self {
            trust_id: trust.trust_id,
            local_laboratory_id: trust.local_laboratory_id,
            remote_node_id: trust.remote_node_id,
            remote_base_url: remote.base_url,
            remote_laboratory_id: trust.remote_laboratory_id,
            remote_laboratory_name: trust.remote_laboratory_name,
            status: trust.status,
            created_at: trust.created_at,
            updated_at: trust.updated_at,
            revoked_at: trust.revoked_at,
        }
    }
}

#[derive(sqlx::FromRow)]
pub(super) struct PairingCodeRow {
    pub(super) pairing_code_id: Uuid,
    pub(super) local_laboratory_id: Uuid,
    pub(super) expires_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub(super) struct GuestLinkRow {
    pub(super) link_id: Uuid,
    pub(super) local_laboratory_id: Uuid,
    pub(super) remote_node_id: Uuid,
    pub(super) remote_laboratory_id: Uuid,
    pub(super) remote_user_id: Uuid,
    pub(super) remote_username: String,
    pub(super) remote_user_type: String,
    pub(super) local_guest_user_id: Uuid,
    pub(super) first_seen_at: DateTime<Utc>,
    pub(super) last_seen_at: DateTime<Utc>,
    pub(super) local_guest_username: String,
    pub(super) remote_base_url: String,
}

#[derive(Serialize)]
pub(super) struct GuestLinkResponse {
    link_id: Uuid,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_base_url: String,
    remote_laboratory_id: Uuid,
    remote_user_id: Uuid,
    remote_username: String,
    remote_user_type: String,
    local_guest_user_id: Uuid,
    local_guest_username: String,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

impl From<GuestLinkRow> for GuestLinkResponse {
    fn from(row: GuestLinkRow) -> Self {
        Self {
            link_id: row.link_id,
            local_laboratory_id: row.local_laboratory_id,
            remote_node_id: row.remote_node_id,
            remote_base_url: row.remote_base_url,
            remote_laboratory_id: row.remote_laboratory_id,
            remote_user_id: row.remote_user_id,
            remote_username: row.remote_username,
            remote_user_type: row.remote_user_type,
            local_guest_user_id: row.local_guest_user_id,
            local_guest_username: row.local_guest_username,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
        }
    }
}

#[derive(sqlx::FromRow)]
pub(super) struct LaboratoryIdentityRow {
    pub(super) laboratory_id: Uuid,
    pub(super) name: String,
}

/// The local user a proxied request is made on behalf of, whose name and role
/// are carried to the remote node in the signed headers.
#[derive(sqlx::FromRow)]
pub(super) struct ProxyUserRow {
    pub(super) user_id: Uuid,
    pub(super) username: String,
    pub(super) user_type_name: String,
    pub(super) laboratory_id: Option<Uuid>,
}

pub(super) fn trust_audit_details(trust: &TrustRow) -> Value {
    json!({
        "trust_id": trust.trust_id,
        "local_laboratory_id": trust.local_laboratory_id,
        "remote_node_id": trust.remote_node_id,
        "remote_laboratory_id": trust.remote_laboratory_id,
        "status": trust.status,
    })
}

pub(super) fn guest_link_audit_details(link_id: Uuid, target_guest_user_id: Uuid) -> Value {
    json!({
        "link_id": link_id,
        "target_guest_user_id": target_guest_user_id,
    })
}
