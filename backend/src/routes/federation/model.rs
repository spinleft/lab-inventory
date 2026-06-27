use crate::configuration::FederationSettings;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

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

pub async fn initialize_local_node(
    pool: &PgPool,
    settings: &FederationSettings,
) -> Result<(), anyhow::Error> {
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to begin local federation node initialization")?;
    let row = fetch_local_node_for_update(&mut transaction).await?;
    match row {
        Some(row) => {
            if row.public_base_url != settings.public_base_url {
                sqlx::query(
                    r#"
                    UPDATE federation_local_nodes
                    SET public_base_url = $2,
                        updated_at = now()
                    WHERE node_id = $1
                    "#,
                )
                .bind(row.node_id)
                .bind(&settings.public_base_url)
                .execute(transaction.as_mut())
                .await
                .context("Failed to update federation local node public_base_url")?;
                LocalNodeRow {
                    public_base_url: settings.public_base_url.clone(),
                    ..row
                }
            } else {
                row
            }
        }
        None => {
            let node_id = Uuid::new_v4();
            sqlx::query_as::<_, LocalNodeRow>(
                r#"
                INSERT INTO federation_local_nodes (node_id, public_base_url)
                VALUES ($1, $2)
                RETURNING node_id, public_base_url
                "#,
            )
            .bind(node_id)
            .bind(&settings.public_base_url)
            .fetch_one(transaction.as_mut())
            .await
            .context("Failed to insert federation local node")?
        }
    };
    transaction
        .commit()
        .await
        .context("Failed to commit local federation node initialization")?;
    Ok(())
}

pub(super) async fn fetch_local_node(pool: &PgPool) -> Result<LocalNodeRow, FederationError> {
    sqlx::query_as::<_, LocalNodeRow>(
        r#"
        SELECT node_id, public_base_url
        FROM federation_local_nodes
        ORDER BY created_at
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::UnexpectedError(anyhow::anyhow!("Local node not initialized")))
}

async fn fetch_local_node_for_update(
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Option<LocalNodeRow>, anyhow::Error> {
    sqlx::query_as::<_, LocalNodeRow>(
        r#"
        SELECT node_id, public_base_url
        FROM federation_local_nodes
        ORDER BY created_at
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch federation local node")
}

pub(super) async fn fetch_laboratory_identity(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<LaboratoryIdentityRow, FederationError> {
    sqlx::query_as::<_, LaboratoryIdentityRow>(
        r#"
        SELECT laboratory_id, name
        FROM laboratories
        WHERE laboratory_id = $1
        "#,
    )
    .bind(laboratory_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Laboratory not found".into()))
}

pub(super) async fn fetch_remote_node(
    pool: &PgPool,
    remote_node_id: Uuid,
) -> Result<RemoteNodeRow, FederationError> {
    sqlx::query_as::<_, RemoteNodeRow>(
        r#"
        SELECT
            remote_node_id,
            base_url,
            display_name,
            shared_secret,
            shared_secret_hash,
            tls_certificate_sha256,
            status,
            key_version,
            last_handshake_at
        FROM federation_remote_nodes
        WHERE remote_node_id = $1
        "#,
    )
    .bind(remote_node_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::Unauthorized("Unknown federation node".into()))
}

pub(super) async fn fetch_active_trust(
    pool: &PgPool,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_laboratory_id: Uuid,
) -> Result<TrustRow, FederationError> {
    sqlx::query_as::<_, TrustRow>(
        r#"
        SELECT
            trust_id,
            local_laboratory_id,
            remote_node_id,
            remote_laboratory_id,
            remote_laboratory_name,
            status,
            created_at,
            updated_at,
            revoked_at
        FROM federation_laboratory_trusts
        WHERE local_laboratory_id = $1
          AND remote_node_id = $2
          AND remote_laboratory_id = $3
          AND status = 'active'
        "#,
    )
    .bind(local_laboratory_id)
    .bind(remote_node_id)
    .bind(remote_laboratory_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::Forbidden("Laboratory trust is not active".into()))
}

pub(super) async fn upsert_remote_node(
    transaction: &mut Transaction<'_, Postgres>,
    remote_node_id: Uuid,
    base_url: &str,
    display_name: Option<&str>,
    shared_secret: &str,
    shared_secret_hash: &str,
    tls_certificate_sha256: Option<&str>,
    key_version: i32,
) -> Result<RemoteNodeRow, FederationError> {
    sqlx::query_as::<_, RemoteNodeRow>(
        r#"
        INSERT INTO federation_remote_nodes (
            remote_node_id,
            base_url,
            display_name,
            shared_secret,
            shared_secret_hash,
            tls_certificate_sha256,
            key_version,
            last_handshake_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, now())
        ON CONFLICT (remote_node_id)
        DO UPDATE SET
            base_url = EXCLUDED.base_url,
            display_name = EXCLUDED.display_name,
            shared_secret = EXCLUDED.shared_secret,
            shared_secret_hash = EXCLUDED.shared_secret_hash,
            tls_certificate_sha256 = EXCLUDED.tls_certificate_sha256,
            key_version = EXCLUDED.key_version,
            status = 'active',
            last_handshake_at = now(),
            updated_at = now()
        RETURNING
            remote_node_id,
            base_url,
            display_name,
            shared_secret,
            shared_secret_hash,
            tls_certificate_sha256,
            status,
            key_version,
            last_handshake_at
        "#,
    )
    .bind(remote_node_id)
    .bind(base_url)
    .bind(display_name)
    .bind(shared_secret)
    .bind(shared_secret_hash)
    .bind(tls_certificate_sha256)
    .bind(key_version)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

pub(super) async fn upsert_trust(
    transaction: &mut Transaction<'_, Postgres>,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_laboratory_id: Uuid,
    remote_laboratory_name: Option<&str>,
    created_by_user_id: Option<Uuid>,
) -> Result<TrustRow, FederationError> {
    sqlx::query_as::<_, TrustRow>(
        r#"
        INSERT INTO federation_laboratory_trusts (
            trust_id,
            local_laboratory_id,
            remote_node_id,
            remote_laboratory_id,
            remote_laboratory_name,
            created_by_user_id
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (local_laboratory_id, remote_node_id, remote_laboratory_id)
        DO UPDATE SET
            remote_laboratory_name = EXCLUDED.remote_laboratory_name,
            status = 'active',
            revoked_at = NULL,
            updated_at = now()
        RETURNING
            trust_id,
            local_laboratory_id,
            remote_node_id,
            remote_laboratory_id,
            remote_laboratory_name,
            status,
            created_at,
            updated_at,
            revoked_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(local_laboratory_id)
    .bind(remote_node_id)
    .bind(remote_laboratory_id)
    .bind(remote_laboratory_name)
    .bind(created_by_user_id)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
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

fn map_database_error(error: sqlx::Error) -> FederationError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => {
                return FederationError::ConflictError("Federation record already exists".into());
            }
            Some("23503") => {
                return FederationError::ValidationError("Invalid referenced record".into());
            }
            Some("23514") => {
                return FederationError::ValidationError("Invalid federation data".into());
            }
            _ => {}
        }
    }
    FederationError::UnexpectedError(error.into())
}
