//! Every SQL statement the federation routes issue lives here, except the
//! read-only public API, which has its own `public_data::queries`.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration;
//!   anything that chains several statements belongs in `service.rs`
//! - functions never return a handler error type, only [`FederationError`],
//!   which every federation route already answers with
use super::model::{
    FederationError, GuestLinkRow, LaboratoryIdentityRow, LocalNodeRow, PairingCodeRow,
    ProxyUserRow, RemoteNodeRow, TrustRow, TrustWithRemoteRow,
};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(super) fn map_database_error(error: sqlx::Error) -> FederationError {
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

fn unexpected(error: sqlx::Error) -> FederationError {
    FederationError::UnexpectedError(error.into())
}

// ---------------------------------------------------------------------------
// this node
// ---------------------------------------------------------------------------

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
    .map_err(unexpected)?
    .ok_or_else(|| FederationError::UnexpectedError(anyhow::anyhow!("Local node not initialized")))
}

pub(super) async fn fetch_local_node_for_update(
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Option<LocalNodeRow>, FederationError> {
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
    .map_err(unexpected)
}

pub(super) async fn insert_local_node(
    transaction: &mut Transaction<'_, Postgres>,
    public_base_url: &str,
) -> Result<(), FederationError> {
    sqlx::query(
        r#"
        INSERT INTO federation_local_nodes (node_id, public_base_url)
        VALUES ($1, $2)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(public_base_url)
    .execute(transaction.as_mut())
    .await
    .map_err(unexpected)?;

    Ok(())
}

pub(super) async fn update_local_node_base_url(
    transaction: &mut Transaction<'_, Postgres>,
    node_id: Uuid,
    public_base_url: &str,
) -> Result<(), FederationError> {
    sqlx::query(
        r#"
        UPDATE federation_local_nodes
        SET public_base_url = $2,
            updated_at = now()
        WHERE node_id = $1
        "#,
    )
    .bind(node_id)
    .bind(public_base_url)
    .execute(transaction.as_mut())
    .await
    .map_err(unexpected)?;

    Ok(())
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
    .map_err(unexpected)?
    .ok_or_else(|| FederationError::NotFound("Laboratory not found".into()))
}

// ---------------------------------------------------------------------------
// remote nodes and trusts
// ---------------------------------------------------------------------------

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
    .map_err(unexpected)?
    .ok_or_else(|| FederationError::Unauthorized("Unknown federation node".into()))
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

/// Re-pairing a laboratory pair that was revoked earlier brings the trust back
/// rather than leaving a dead row behind.
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
    .map_err(unexpected)?
    .ok_or_else(|| FederationError::Forbidden("Laboratory trust is not active".into()))
}

pub(super) async fn fetch_trusts(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Vec<TrustWithRemoteRow>, FederationError> {
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
    .map_err(unexpected)
}

/// A trust is revoked rather than deleted, so the history of the pairing stays
/// on record.
pub(super) async fn revoke_trust_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    trust_id: Uuid,
) -> Result<TrustRow, FederationError> {
    sqlx::query_as::<_, TrustRow>(
        r#"
        UPDATE federation_laboratory_trusts
        SET status = 'revoked',
            revoked_at = now(),
            updated_at = now()
        WHERE local_laboratory_id = $1
          AND trust_id = $2
        RETURNING trust_id, local_laboratory_id, remote_node_id, remote_laboratory_id, remote_laboratory_name, status, created_at, updated_at, revoked_at
        "#,
    )
    .bind(laboratory_id)
    .bind(trust_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(unexpected)?
    .ok_or_else(|| FederationError::NotFound("Federation trust not found".into()))
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
) -> Result<PairingCodeRow, FederationError> {
    sqlx::query_as::<_, PairingCodeRow>(
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
    )
    .bind(Uuid::new_v4())
    .bind(laboratory_id)
    .bind(code_hash)
    .bind(expires_at)
    .bind(created_by_user_id)
    .fetch_one(pool)
    .await
    .map_err(unexpected)
}

/// Claims a pairing code, in a single statement so two requests presenting the
/// same code cannot both succeed.
pub(super) async fn consume_pairing_code(
    transaction: &mut Transaction<'_, Postgres>,
    code_hash: &str,
) -> Result<PairingCodeRow, FederationError> {
    sqlx::query_as::<_, PairingCodeRow>(
        r#"
        UPDATE federation_pairing_codes
        SET consumed_at = now()
        WHERE pairing_code_id = (
            SELECT pairing_code_id
            FROM federation_pairing_codes
            WHERE code_hash = $1
              AND consumed_at IS NULL
              AND expires_at > now()
            FOR UPDATE
        )
        RETURNING pairing_code_id, local_laboratory_id, expires_at
        "#,
    )
    .bind(code_hash)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(unexpected)?
    .ok_or_else(|| FederationError::Unauthorized("Pairing code is invalid or expired".into()))
}

// ---------------------------------------------------------------------------
// request nonces
// ---------------------------------------------------------------------------

pub(super) async fn delete_expired_nonces(pool: &PgPool) -> Result<(), FederationError> {
    sqlx::query("DELETE FROM federation_request_nonces WHERE expires_at <= now()")
        .execute(pool)
        .await
        .map_err(unexpected)?;

    Ok(())
}

/// Records a nonce so the same signed request cannot be replayed. A duplicate is
/// exactly the replay this guards against, hence the dedicated error.
pub(super) async fn insert_nonce(
    pool: &PgPool,
    remote_node_id: Uuid,
    nonce: &str,
    ttl_seconds: i64,
) -> Result<(), FederationError> {
    let result = sqlx::query(
        r#"
        INSERT INTO federation_request_nonces (remote_node_id, nonce, expires_at)
        VALUES ($1, $2, now() + ($3 || ' seconds')::interval)
        "#,
    )
    .bind(remote_node_id)
    .bind(nonce)
    .bind(ttl_seconds.to_string())
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => Err(
            FederationError::Unauthorized("Federation request nonce has already been used".into()),
        ),
        Err(error) => Err(unexpected(error)),
    }
}

// ---------------------------------------------------------------------------
// guest links and their shadow users
// ---------------------------------------------------------------------------

fn guest_link_select(suffix: &str) -> String {
    format!(
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
        {suffix}
        "#
    )
}

pub(super) async fn fetch_guest_links(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Vec<GuestLinkRow>, FederationError> {
    sqlx::query_as::<_, GuestLinkRow>(&guest_link_select(
        "WHERE links.local_laboratory_id = $1 ORDER BY links.last_seen_at DESC, links.link_id",
    ))
    .bind(laboratory_id)
    .fetch_all(pool)
    .await
    .map_err(unexpected)
}

pub(super) async fn fetch_guest_link(
    pool: &PgPool,
    laboratory_id: Uuid,
    link_id: Uuid,
) -> Result<GuestLinkRow, FederationError> {
    sqlx::query_as::<_, GuestLinkRow>(&guest_link_select(
        "WHERE links.local_laboratory_id = $1 AND links.link_id = $2",
    ))
    .bind(laboratory_id)
    .bind(link_id)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)?
    .ok_or_else(|| FederationError::NotFound("Federation guest link not found".into()))
}

pub(super) async fn fetch_guest_link_user_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    link_id: Uuid,
) -> Result<Uuid, FederationError> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT local_guest_user_id
        FROM federation_guest_links
        WHERE local_laboratory_id = $1
          AND link_id = $2
        FOR UPDATE
        "#,
    )
    .bind(laboratory_id)
    .bind(link_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(unexpected)?
    .ok_or_else(|| FederationError::NotFound("Federation guest link not found".into()))
}

pub(super) async fn update_guest_link_user(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    link_id: Uuid,
    target_guest_user_id: Uuid,
) -> Result<(), FederationError> {
    sqlx::query(
        r#"
        UPDATE federation_guest_links
        SET local_guest_user_id = $3,
            last_seen_at = now()
        WHERE local_laboratory_id = $1
          AND link_id = $2
        "#,
    )
    .bind(laboratory_id)
    .bind(link_id)
    .bind(target_guest_user_id)
    .execute(transaction.as_mut())
    .await
    .map_err(unexpected)?;

    Ok(())
}

/// Refreshes the link a remote user already has here, returning the local guest
/// they are known as. `None` means this remote user has never been seen.
pub(super) async fn touch_guest_link(
    pool: &PgPool,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_laboratory_id: Uuid,
    remote_user_id: Uuid,
    remote_username: &str,
    remote_user_type: &str,
) -> Result<Option<Uuid>, FederationError> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        UPDATE federation_guest_links
        SET remote_username = $5,
            remote_user_type = $6,
            last_seen_at = now()
        WHERE local_laboratory_id = $1
          AND remote_node_id = $2
          AND remote_laboratory_id = $3
          AND remote_user_id = $4
        RETURNING local_guest_user_id
        "#,
    )
    .bind(local_laboratory_id)
    .bind(remote_node_id)
    .bind(remote_laboratory_id)
    .bind(remote_user_id)
    .bind(remote_username)
    .bind(remote_user_type)
    .fetch_optional(pool)
    .await
    .map_err(unexpected)
}

/// Returns the local guest the link ended up pointing at, which is not
/// necessarily `local_guest_user_id`: a concurrent request may have created the
/// link first, and its guest wins.
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
) -> Result<Uuid, FederationError> {
    sqlx::query_scalar::<_, Uuid>(
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
        WHERE remote_node_id IS NOT NULL
        DO UPDATE SET
            remote_username = EXCLUDED.remote_username,
            remote_user_type = EXCLUDED.remote_user_type,
            last_seen_at = now()
        RETURNING local_guest_user_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(local_laboratory_id)
    .bind(remote_node_id)
    .bind(remote_laboratory_id)
    .bind(remote_user_id)
    .bind(remote_username)
    .bind(remote_user_type)
    .bind(local_guest_user_id)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(unexpected)
}

pub(super) async fn guest_link_exists_for_user(
    transaction: &mut Transaction<'_, Postgres>,
    local_guest_user_id: Uuid,
) -> Result<bool, FederationError> {
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
    .map_err(unexpected)
}

/// A shadow guest exists only to stand in for a remote user, so it can be
/// dropped once nothing points at it. The `is_federation_shadow` guard keeps a
/// real account from ever being deleted here.
pub(super) async fn delete_shadow_guest(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<(), FederationError> {
    sqlx::query(
        r#"
        DELETE FROM users
        WHERE user_id = $1
          AND is_federation_shadow = true
        "#,
    )
    .bind(user_id)
    .execute(transaction.as_mut())
    .await
    .map_err(unexpected)?;

    Ok(())
}

pub(super) async fn insert_shadow_guest(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    username: &str,
    password_hash: &str,
    local_laboratory_id: Uuid,
) -> Result<Uuid, FederationError> {
    sqlx::query_scalar::<_, Uuid>(
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
    )
    .bind(user_id)
    .bind(username)
    .bind(password_hash)
    .bind(local_laboratory_id)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(unexpected)
}

// ---------------------------------------------------------------------------
// local users
// ---------------------------------------------------------------------------

pub(super) async fn fetch_proxy_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<ProxyUserRow, FederationError> {
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
    .map_err(unexpected)?
    .ok_or_else(|| FederationError::Forbidden("Current user not found".into()))
}

pub(super) async fn fetch_user_role(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<(String, Option<Uuid>)>, FederationError> {
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
    .map_err(unexpected)
}
