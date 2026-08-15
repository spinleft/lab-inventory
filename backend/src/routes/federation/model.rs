use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

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

/// Who an inbound federated caller is, once resolved against this server: the
/// link that records their remote identity, and the local guest account they act
/// as here.
///
/// Anything a federated caller writes is attributed through both. The account is
/// what the local authorization rules see; the link is what the record is filed
/// under, because a link outlives the account it points at.
#[derive(Clone, Copy, Debug)]
pub(super) struct GuestLinkIdentity {
    pub(super) link_id: Uuid,
    pub(super) local_guest_user_id: Uuid,
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
