//! Business flows that chain several statements together, and the rules about
//! who may act on federation state.
//!
//! Anything here orchestrates `queries.rs`. Single-statement work belongs in
//! `queries.rs`; HTTP concerns belong in the handler modules.
use super::model::FederationError;
use super::queries::{
    delete_expired_nonces, delete_shadow_guest, fetch_local_node_for_update, fetch_user_role,
    guest_link_exists_for_user, insert_guest_link, insert_local_node, insert_nonce,
    insert_shadow_guest, touch_guest_link, update_local_node_base_url,
};
use super::security::InboundFederationContext;
use crate::access_control::{Actor, get_actor};
use crate::authentication::hash_password;
use crate::configuration::FederationSettings;
use crate::domain::{LaboratoryId, UserId, UserType};
use secrecy::{ExposeSecret, Secret};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Makes sure this server has exactly one federation identity, and that the
/// base URL other nodes reach it on matches the configuration.
pub async fn initialize_local_node(
    pool: &PgPool,
    settings: &FederationSettings,
) -> Result<(), anyhow::Error> {
    let mut transaction = pool.begin().await?;
    match fetch_local_node_for_update(&mut transaction).await? {
        Some(row) if row.public_base_url != settings.public_base_url => {
            update_local_node_base_url(&mut transaction, row.node_id, &settings.public_base_url)
                .await?;
        }
        Some(_) => {}
        None => insert_local_node(&mut transaction, &settings.public_base_url).await?,
    }
    transaction.commit().await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// who may act
// ---------------------------------------------------------------------------

pub(super) async fn lab_admin_for_laboratory(
    pool: &PgPool,
    actor_user_id: UserId,
    laboratory_id: Uuid,
) -> Result<Actor, FederationError> {
    let actor = federation_actor(pool, actor_user_id).await?;
    let laboratory_id = parse_laboratory_id(laboratory_id)?;
    if actor.is_lab_admin() && actor.laboratory_id == Some(laboratory_id) {
        Ok(actor)
    } else {
        Err(FederationError::Forbidden(
            "Only this laboratory's administrator can manage federation".into(),
        ))
    }
}

pub(super) async fn federation_reader_for_laboratory(
    pool: &PgPool,
    actor_user_id: UserId,
    laboratory_id: Uuid,
) -> Result<Actor, FederationError> {
    let actor = federation_actor(pool, actor_user_id).await?;
    let laboratory_id = parse_laboratory_id(laboratory_id)?;
    if (actor.is_lab_admin() || actor.is_regular_user())
        && actor.laboratory_id == Some(laboratory_id)
    {
        Ok(actor)
    } else {
        Err(FederationError::Forbidden(
            "Only this laboratory's administrators and users can view federation".into(),
        ))
    }
}

pub(super) async fn federation_actor(
    pool: &PgPool,
    actor_user_id: UserId,
) -> Result<Actor, FederationError> {
    get_actor(pool, actor_user_id)
        .await
        .map_err(FederationError::UnexpectedError)?
        .ok_or_else(|| FederationError::Forbidden("Actor not found in the database".into()))
}

fn parse_laboratory_id(laboratory_id: Uuid) -> Result<LaboratoryId, FederationError> {
    LaboratoryId::parse(laboratory_id)
        .map_err(|e| FederationError::UnexpectedError(anyhow::anyhow!("{e}")))
}

/// A guest link may only be pointed at a guest account of the same laboratory:
/// merging it onto anything else would hand a remote user someone's identity.
pub(super) async fn validate_target_guest(
    pool: &PgPool,
    laboratory_id: Uuid,
    target_guest_user_id: Uuid,
) -> Result<(), FederationError> {
    let Some((user_type, user_laboratory_id)) = fetch_user_role(pool, target_guest_user_id).await?
    else {
        return Err(FederationError::ValidationError(
            "Target guest user not found".into(),
        ));
    };
    if UserType::parse(&user_type).map_err(FederationError::ValidationError)? != UserType::Guest
        || user_laboratory_id != Some(laboratory_id)
    {
        return Err(FederationError::ValidationError(
            "Target user must be a guest in this laboratory".into(),
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// guest links
// ---------------------------------------------------------------------------

/// The local guest account a remote user acts as in this laboratory.
///
/// A user seen before keeps the account they already have. A new one gets a
/// shadow account created for them, which exists only to give their activity a
/// local identity.
pub(super) async fn upsert_guest_link(
    pool: &PgPool,
    local_laboratory_id: Uuid,
    context: &InboundFederationContext,
) -> Result<Uuid, FederationError> {
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
    if let Some(user_id) = existing {
        return Ok(user_id);
    }

    // The shadow account can never be signed into directly, but the column is
    // not nullable, so it is given a password nobody knows.
    let password_hash = hash_password(Secret::new(super::security::generate_token(32)))
        .await
        .map_err(FederationError::UnexpectedError)?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let guest_user_id = insert_shadow_guest(
        &mut transaction,
        Uuid::new_v4(),
        &shadow_guest_username(context),
        password_hash.expose_secret(),
        local_laboratory_id,
    )
    .await?;
    let link_user_id = insert_guest_link(
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
    if link_user_id != guest_user_id {
        delete_shadow_guest(&mut transaction, guest_user_id).await?;
    }
    transaction
        .commit()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;

    Ok(link_user_id)
}

fn shadow_guest_username(context: &InboundFederationContext) -> String {
    let node = context.remote_node.remote_node_id.to_string();
    let user = context.remote_user_id.to_string();
    format!("fed_{}_{}", &node[..8], &user[..8])
}

/// Points a guest link at another local account, cleaning up the shadow account
/// it used to use if that leaves it unreferenced.
pub(super) async fn merge_guest_link_user(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    link_id: Uuid,
    target_guest_user_id: Uuid,
) -> Result<(), FederationError> {
    let old_guest_user_id = super::queries::fetch_guest_link_user_for_update(
        transaction,
        laboratory_id,
        link_id,
    )
    .await?;
    super::queries::update_guest_link_user(
        transaction,
        laboratory_id,
        link_id,
        target_guest_user_id,
    )
    .await?;
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
) -> Result<(), FederationError> {
    delete_expired_nonces(pool).await?;
    insert_nonce(pool, remote_node_id, nonce, ttl_seconds).await
}
