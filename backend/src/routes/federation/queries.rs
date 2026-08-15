//! Every SQL statement the federation routes issue lives here, except the
//! read-only public API, which has its own `public_data::queries`.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - functions never return a handler error type. A statement that can only
//!   fail unexpectedly returns [`anyhow::Error`], and reports a row that is not
//!   there as `None` so the caller decides what missing means; a statement a
//!   caller can violate returns [`FederationDatabaseError`]
use super::model::{
    GuestLinkIdentity, GuestLinkRow, LaboratoryIdentityRow, PairingCodeRow, ProxyUserRow,
    RemoteNodeRow, TrustRow, TrustWithRemoteRow,
};
use crate::utils::error_chain_fmt;
use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(super) enum FederationDatabaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for FederationDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> FederationDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match database_error.code().as_deref() {
            Some("23505") => {
                return FederationDatabaseError::Conflict(
                    "Federation record already exists".into(),
                );
            }
            Some("23503") => {
                return FederationDatabaseError::Validation("Invalid referenced record".into());
            }
            Some("23514") => {
                return FederationDatabaseError::Validation("Invalid federation data".into());
            }
            _ => {}
        }
    }

    FederationDatabaseError::Unexpected(error.into())
}

// ---------------------------------------------------------------------------
// this node
// ---------------------------------------------------------------------------

pub(super) async fn fetch_local_node_id(pool: &PgPool) -> Result<Option<Uuid>, anyhow::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT node_id
        FROM federation_local_nodes
        "#,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch node id")
}

pub(super) async fn fetch_laboratory_identity(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Option<LaboratoryIdentityRow>, anyhow::Error> {
    sqlx::query_as!(
        LaboratoryIdentityRow,
        r#"
        SELECT laboratory_id, name
        FROM laboratories
        WHERE laboratory_id = $1
        "#,
        laboratory_id
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch laboratory identity")
}

// ---------------------------------------------------------------------------
// remote nodes and trusts
// ---------------------------------------------------------------------------

pub(super) async fn fetch_remote_node(
    pool: &PgPool,
    remote_node_id: Uuid,
) -> Result<Option<RemoteNodeRow>, anyhow::Error> {
    sqlx::query_as!(
        RemoteNodeRow,
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
        remote_node_id
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch remote node")
}

/// Pairing may be repeated to rotate the shared secret, so a node that is
/// already known is refreshed rather than rejected.
#[allow(clippy::too_many_arguments)]
pub(super) async fn upsert_remote_node(
    transaction: &mut Transaction<'_, Postgres>,
    remote_node_id: Uuid,
    base_url: &str,
    display_name: Option<&str>,
    shared_secret: &str,
    shared_secret_hash: &str,
    tls_certificate_sha256: Option<&str>,
    key_version: i32,
) -> Result<RemoteNodeRow, FederationDatabaseError> {
    sqlx::query_as!(
        RemoteNodeRow,
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
        remote_node_id,
        base_url,
        display_name,
        shared_secret,
        shared_secret_hash,
        tls_certificate_sha256,
        key_version
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

/// Re-pairing a laboratory pair that was revoked earlier brings the trust back
/// rather than leaving a dead row behind.
pub(super) async fn upsert_trust(
    transaction: &mut Transaction<'_, Postgres>,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_laboratory_id: Uuid,
    remote_laboratory_name: Option<&str>,
    created_by_user_id: Option<Uuid>,
) -> Result<TrustRow, FederationDatabaseError> {
    sqlx::query_as!(
        TrustRow,
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
        Uuid::new_v4(),
        local_laboratory_id,
        remote_node_id,
        remote_laboratory_id,
        remote_laboratory_name,
        created_by_user_id
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

pub(super) async fn fetch_active_trust(
    pool: &PgPool,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_laboratory_id: Uuid,
) -> Result<Option<TrustRow>, anyhow::Error> {
    sqlx::query_as!(
        TrustRow,
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
        local_laboratory_id,
        remote_node_id,
        remote_laboratory_id
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch federation trust")
}

pub(super) async fn fetch_trusts(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Vec<TrustWithRemoteRow>, anyhow::Error> {
    sqlx::query_as::<_, TrustWithRemoteRow>(
        r#"
        SELECT
            trusts.trust_id,
            trusts.local_laboratory_id,
            trusts.remote_node_id,
            trusts.remote_laboratory_id,
            trusts.remote_laboratory_name,
            trusts.status,
            trusts.created_at,
            trusts.updated_at,
            trusts.revoked_at,
            nodes.base_url AS remote_base_url
        FROM federation_laboratory_trusts AS trusts
        JOIN federation_remote_nodes AS nodes
          ON nodes.remote_node_id = trusts.remote_node_id
        WHERE trusts.local_laboratory_id = $1
        ORDER BY trusts.created_at DESC, trusts.trust_id
        "#,
    )
    .bind(laboratory_id)
    .fetch_all(pool)
    .await
    .context("Failed to fetch federation trusts")
}

/// A trust is revoked rather than deleted, so the history of the pairing stays
/// on record.
pub(super) async fn revoke_trust_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    trust_id: Uuid,
) -> Result<Option<TrustRow>, FederationDatabaseError> {
    sqlx::query_as!(
        TrustRow,
        r#"
        UPDATE federation_laboratory_trusts
        SET status = 'revoked',
            revoked_at = now(),
            updated_at = now()
        WHERE local_laboratory_id = $1
          AND trust_id = $2
        RETURNING trust_id, local_laboratory_id, remote_node_id, remote_laboratory_id, remote_laboratory_name, status, created_at, updated_at, revoked_at
        "#,
        laboratory_id,
        trust_id
    )
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

// ---------------------------------------------------------------------------
// pairing codes
// ---------------------------------------------------------------------------

pub(super) async fn insert_pairing_code(
    pool: &PgPool,
    laboratory_id: Uuid,
    code_hash: &str,
    expires_at: DateTime<Utc>,
    created_by_user_id: Uuid,
) -> Result<PairingCodeRow, FederationDatabaseError> {
    sqlx::query_as!(
        PairingCodeRow,
        r#"
        INSERT INTO federation_pairing_codes (
            pairing_code_id,
            local_laboratory_id,
            code_hash,
            expires_at,
            created_by_user_id
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING pairing_code_id, local_laboratory_id, expires_at
        "#,
        Uuid::new_v4(),
        laboratory_id,
        code_hash,
        expires_at,
        created_by_user_id
    )
    .fetch_one(pool)
    .await
    .map_err(map_database_error)
}

/// Claims a pairing code, in a single statement so two requests presenting the
/// same code cannot both succeed.
pub(super) async fn consume_pairing_code(
    transaction: &mut Transaction<'_, Postgres>,
    code_hash: &str,
) -> Result<Option<PairingCodeRow>, FederationDatabaseError> {
    sqlx::query_as!(
        PairingCodeRow,
        r#"
        UPDATE federation_pairing_codes
        SET consumed_at = now()
        WHERE code_hash = $1
          AND consumed_at IS NULL
          AND expires_at > now()
        RETURNING pairing_code_id, local_laboratory_id, expires_at
        "#,
        code_hash
    )
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

// ---------------------------------------------------------------------------
// request nonces
// ---------------------------------------------------------------------------

pub(super) async fn delete_expired_nonces(pool: &PgPool) -> Result<(), FederationDatabaseError> {
    sqlx::query("DELETE FROM federation_request_nonces WHERE expires_at <= now()")
        .execute(pool)
        .await
        .map_err(map_database_error)?;

    Ok(())
}

/// Records a nonce so the same signed request cannot be replayed. A duplicate is
/// exactly the replay this guards against, hence the dedicated message.
pub(super) async fn insert_nonce(
    pool: &PgPool,
    remote_node_id: Uuid,
    nonce: &str,
    ttl_seconds: i64,
) -> Result<(), FederationDatabaseError> {
    sqlx::query!(
        r#"
        INSERT INTO federation_request_nonces (remote_node_id, nonce, expires_at)
        VALUES ($1, $2, now() + ($3::BIGINT * INTERVAL '1 second'))
        "#,
        remote_node_id,
        nonce,
        ttl_seconds
    )
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|error| match map_database_error(error) {
        FederationDatabaseError::Conflict(_) => FederationDatabaseError::Conflict(
            "Federation request nonce has already been used".into(),
        ),
        other => other,
    })
}

// ---------------------------------------------------------------------------
// guest links and their shadow users
// ---------------------------------------------------------------------------

pub(super) async fn fetch_guest_links(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Vec<GuestLinkRow>, anyhow::Error> {
    sqlx::query_as!(
        GuestLinkRow,
        r#"
        SELECT
            links.link_id,
            links.local_laboratory_id,
            links.remote_node_id,
            links.remote_laboratory_id,
            links.remote_user_id,
            links.remote_username,
            links.remote_user_type,
            links.local_guest_user_id,
            links.first_seen_at,
            links.last_seen_at,
            users.username AS local_guest_username,
            nodes.base_url AS remote_base_url
        FROM federation_guest_links AS links
        JOIN users ON users.user_id = links.local_guest_user_id
        JOIN federation_remote_nodes AS nodes ON nodes.remote_node_id = links.remote_node_id
        WHERE links.local_laboratory_id = $1
        ORDER BY links.last_seen_at DESC, links.link_id
        "#,
        laboratory_id
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch federation guest links")
}

pub(super) async fn fetch_guest_link(
    pool: &PgPool,
    laboratory_id: Uuid,
    link_id: Uuid,
) -> Result<Option<GuestLinkRow>, anyhow::Error> {
    sqlx::query_as!(
        GuestLinkRow,
        r#"
        SELECT
            links.link_id,
            links.local_laboratory_id,
            links.remote_node_id,
            links.remote_laboratory_id,
            links.remote_user_id,
            links.remote_username,
            links.remote_user_type,
            links.local_guest_user_id,
            links.first_seen_at,
            links.last_seen_at,
            users.username AS local_guest_username,
            nodes.base_url AS remote_base_url
        FROM federation_guest_links AS links
        JOIN users ON users.user_id = links.local_guest_user_id
        JOIN federation_remote_nodes AS nodes ON nodes.remote_node_id = links.remote_node_id
        WHERE links.local_laboratory_id = $1 AND links.link_id = $2
        "#,
        laboratory_id,
        link_id
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch federation guest link")
}

pub(super) async fn fetch_guest_link_user_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    link_id: Uuid,
) -> Result<Option<Uuid>, anyhow::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT local_guest_user_id
        FROM federation_guest_links
        WHERE local_laboratory_id = $1
          AND link_id = $2
        FOR UPDATE
        "#,
        laboratory_id,
        link_id
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch federation guest link user")
}

pub(super) async fn update_guest_link_user(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    link_id: Uuid,
    target_guest_user_id: Uuid,
) -> Result<(), FederationDatabaseError> {
    sqlx::query!(
        r#"
        UPDATE federation_guest_links
        SET local_guest_user_id = $3,
            last_seen_at = now()
        WHERE local_laboratory_id = $1
          AND link_id = $2
        "#,
        laboratory_id,
        link_id,
        target_guest_user_id
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

/// Refreshes the link a remote user already has here, returning who they are
/// known as. `None` means this remote user has never been seen.
pub(super) async fn touch_guest_link(
    pool: &PgPool,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_laboratory_id: Uuid,
    remote_user_id: Uuid,
    remote_username: &str,
    remote_user_type: &str,
) -> Result<Option<GuestLinkIdentity>, FederationDatabaseError> {
    let row = sqlx::query!(
        r#"
        UPDATE federation_guest_links
        SET remote_username = $5,
            remote_user_type = $6,
            last_seen_at = now()
        WHERE local_laboratory_id = $1
          AND remote_node_id = $2
          AND remote_laboratory_id = $3
          AND remote_user_id = $4
        RETURNING link_id, local_guest_user_id
        "#,
        local_laboratory_id,
        remote_node_id,
        remote_laboratory_id,
        remote_user_id,
        remote_username,
        remote_user_type
    )
    .fetch_optional(pool)
    .await
    .map_err(map_database_error)?;

    Ok(row.map(|row| GuestLinkIdentity {
        link_id: row.link_id,
        local_guest_user_id: row.local_guest_user_id,
    }))
}

/// Returns the link as it stands after the write, whose guest is not necessarily
/// `local_guest_user_id`: a concurrent request may have created the link first,
/// and its guest wins.
#[allow(clippy::too_many_arguments)]
pub(super) async fn insert_guest_link(
    transaction: &mut Transaction<'_, Postgres>,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_laboratory_id: Uuid,
    remote_user_id: Uuid,
    remote_username: &str,
    remote_user_type: &str,
    local_guest_user_id: Uuid,
) -> Result<GuestLinkIdentity, FederationDatabaseError> {
    let row = sqlx::query!(
        r#"
        INSERT INTO federation_guest_links (
            link_id,
            local_laboratory_id,
            remote_node_id,
            remote_laboratory_id,
            remote_user_id,
            remote_username,
            remote_user_type,
            local_guest_user_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (local_laboratory_id, remote_node_id, remote_laboratory_id, remote_user_id)
        DO UPDATE SET
            remote_username = EXCLUDED.remote_username,
            remote_user_type = EXCLUDED.remote_user_type,
            last_seen_at = now()
        RETURNING link_id, local_guest_user_id
        "#,
        Uuid::new_v4(),
        local_laboratory_id,
        remote_node_id,
        remote_laboratory_id,
        remote_user_id,
        remote_username,
        remote_user_type,
        local_guest_user_id
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(GuestLinkIdentity {
        link_id: row.link_id,
        local_guest_user_id: row.local_guest_user_id,
    })
}

pub(super) async fn guest_link_exists_for_user(
    transaction: &mut Transaction<'_, Postgres>,
    local_guest_user_id: Uuid,
) -> Result<bool, anyhow::Error> {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM federation_guest_links
            WHERE local_guest_user_id = $1
        )
        "#,
    )
    .bind(local_guest_user_id)
    .fetch_one(transaction.as_mut())
    .await
    .context("Failed to check whether a federation guest link still exists")
}

/// A shadow guest exists only to stand in for a remote user, so it can be
/// dropped once nothing points at it. The `is_federation_shadow` guard keeps a
/// real account from ever being deleted here.
pub(super) async fn delete_shadow_guest(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<(), FederationDatabaseError> {
    sqlx::query!(
        r#"
        DELETE FROM users
        WHERE user_id = $1
          AND is_federation_shadow = true
        "#,
        user_id
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}

pub(super) async fn insert_shadow_guest(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    username: &str,
    password_hash: &str,
    local_laboratory_id: Uuid,
) -> Result<Uuid, FederationDatabaseError> {
    sqlx::query_scalar!(
        r#"
        INSERT INTO users (
            user_id,
            username,
            password_hash,
            user_type_id,
            laboratory_id,
            is_federation_shadow
        )
        SELECT $1, $2, $3, user_type_id, $4, true
        FROM user_types
        WHERE name = 'guest'
        RETURNING user_id
        "#,
        user_id,
        username,
        password_hash,
        local_laboratory_id
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

// ---------------------------------------------------------------------------
// local users
// ---------------------------------------------------------------------------

pub(super) async fn fetch_proxy_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<ProxyUserRow>, anyhow::Error> {
    sqlx::query_as::<_, ProxyUserRow>(
        r#"
        SELECT users.user_id, users.username, user_types.name AS user_type_name, users.laboratory_id
        FROM users
        JOIN user_types USING (user_type_id)
        WHERE users.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to fetch the user a federation request is proxied for")
}

pub(super) async fn fetch_user_role(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<(String, Option<Uuid>)>, anyhow::Error> {
    sqlx::query_as::<_, (String, Option<Uuid>)>(
        r#"
        SELECT user_types.name, users.laboratory_id
        FROM users
        JOIN user_types USING (user_type_id)
        WHERE users.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to fetch user role")
}
