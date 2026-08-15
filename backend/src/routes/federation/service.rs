//! Business flows that chain several statements together, and the rules about
//! who may act on federation state.
//!
//! Anything here orchestrates `queries.rs`. Single-statement work belongs in
//! `queries.rs`; HTTP concerns belong in the handler modules.
use super::model::GuestLinkIdentity;
use super::queries::{
    FederationDatabaseError, delete_expired_nonces, delete_shadow_guest, fetch_local_node_id,
    fetch_user_role, guest_link_exists_for_user, insert_guest_link, insert_nonce,
    insert_shadow_guest, touch_guest_link, update_guest_link_user,
};
use super::security::{FederationSecurityError, InboundFederationContext};
use crate::authentication::hash_password;
use crate::domain::UserType;
use anyhow::Context;
use secrecy::{ExposeSecret, Secret};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Refuses to start without this server's federation identity.
///
/// The migration mints it, and the table cannot hold a second row, so the only
/// way it can be missing is that someone deleted it. Minting a replacement here
/// would be worse than failing: partners pin the old id, so a new one would
/// strand every trust already built on it while looking like a clean start.
pub async fn initialize_local_node(pool: &PgPool) -> Result<(), anyhow::Error> {
    fetch_local_node_id(pool)
        .await?
        .context("This server has no federation node identity")?;

    Ok(())
}

// ---------------------------------------------------------------------------
// who may act
// ---------------------------------------------------------------------------

// Who may act on federation state is decided by `ResourceType::Federation` in
// `access_control`, alongside every other resource. Each route answers a refusal
// with its own error, and these messages keep the wording the same across all of
// them.

pub(super) const MANAGE_FEDERATION_FORBIDDEN: &str =
    "Only this laboratory's administrator can manage federation";

pub(super) const READ_FEDERATION_FORBIDDEN: &str =
    "Only this laboratory's administrators and users can view federation";

/// A guest link may only be pointed at a guest account of the same laboratory:
/// merging it onto anything else would hand a remote user someone's identity.
pub(super) async fn validate_target_guest(
    pool: &PgPool,
    laboratory_id: Uuid,
    target_guest_user_id: Uuid,
) -> Result<(), FederationDatabaseError> {
    let Some((user_type, user_laboratory_id)) = fetch_user_role(pool, target_guest_user_id).await?
    else {
        return Err(FederationDatabaseError::Validation(
            "Target guest user not found".into(),
        ));
    };
    if UserType::parse(&user_type).map_err(FederationDatabaseError::Validation)? != UserType::Guest
        || user_laboratory_id != Some(laboratory_id)
    {
        return Err(FederationDatabaseError::Validation(
            "Target user must be a guest in this laboratory".into(),
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// guest links
// ---------------------------------------------------------------------------

/// The identity a remote user acts under in this laboratory: the link recording
/// where they came from, and the local guest account standing in for them.
///
/// A user seen before keeps the account they already have. A new one gets a
/// shadow account created for them, which exists only to give their activity a
/// local identity.
pub(super) async fn upsert_guest_link(
    pool: &PgPool,
    local_laboratory_id: Uuid,
    context: &InboundFederationContext,
) -> Result<GuestLinkIdentity, FederationDatabaseError> {
    let existing = touch_guest_link(
        pool,
        local_laboratory_id,
        context.remote_node.remote_node_id,
        context.remote_laboratory_id,
        context.remote_user_id,
        &context.remote_username,
        &context.remote_user_type,
    )
    .await?;
    if let Some(identity) = existing {
        return Ok(identity);
    }

    // The shadow account can never be signed into directly, but the column is
    // not nullable, so it is given a password nobody knows.
    let password_hash = hash_password(Secret::new(super::security::generate_token(32)))
        .await
        .context("Failed to hash the password of a federation shadow guest")?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let guest_user_id = insert_shadow_guest(
        &mut transaction,
        Uuid::new_v4(),
        &shadow_guest_username(context),
        password_hash.expose_secret(),
        local_laboratory_id,
    )
    .await?;
    let identity = insert_guest_link(
        &mut transaction,
        local_laboratory_id,
        context.remote_node.remote_node_id,
        context.remote_laboratory_id,
        context.remote_user_id,
        &context.remote_username,
        &context.remote_user_type,
        guest_user_id,
    )
    .await?;
    // A concurrent request may have created the link first, in which case its
    // guest is the one that counts and the account just created is dead weight.
    if identity.local_guest_user_id != guest_user_id {
        delete_shadow_guest(&mut transaction, guest_user_id).await?;
    }
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a federation guest link")?;

    Ok(identity)
}

fn shadow_guest_username(context: &InboundFederationContext) -> String {
    let node = context.remote_node.remote_node_id.to_string();
    let user = context.remote_user_id.to_string();
    format!("fed_{}_{}", &node[..8], &user[..8])
}

/// Points a guest link at another local account, cleaning up the shadow account
/// it used to use if that leaves it unreferenced.
///
/// `old_guest_user_id` is the one the caller read under `FOR UPDATE`, which is
/// what keeps a concurrent merge from deleting an account this one just linked.
pub(super) async fn merge_guest_link_user(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    link_id: Uuid,
    old_guest_user_id: Uuid,
    target_guest_user_id: Uuid,
) -> Result<(), FederationDatabaseError> {
    update_guest_link_user(transaction, laboratory_id, link_id, target_guest_user_id).await?;
    if !guest_link_exists_for_user(transaction, old_guest_user_id).await? {
        delete_shadow_guest(transaction, old_guest_user_id).await?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// replay protection
// ---------------------------------------------------------------------------

/// Records the nonce of an inbound request, rejecting one that has been seen
/// before. Expired entries are swept first so the table cannot grow forever.
pub(super) async fn remember_nonce(
    pool: &PgPool,
    remote_node_id: Uuid,
    nonce: &str,
    ttl_seconds: i64,
) -> Result<(), FederationSecurityError> {
    delete_expired_nonces(pool).await?;
    insert_nonce(pool, remote_node_id, nonce, ttl_seconds).await?;

    Ok(())
}
