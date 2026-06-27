use super::model::{
    FederationError, PairingCodeRow, fetch_laboratory_identity, fetch_local_node,
    upsert_remote_node, upsert_trust,
};
use super::public_data::{parse_read_target, respond_public_data};
use super::security::{ensure_enabled, normalize_base_url, sha256_hex, verify_inbound_request};
use crate::attachment_storage::AttachmentStorage;
use crate::authentication::hash_password;
use crate::configuration::FederationSettings;
use actix_web::{HttpRequest, HttpResponse, web};
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptPairingBody {
    pairing_code: String,
    requester_node_id: Uuid,
    requester_base_url: String,
    requester_laboratory_id: Uuid,
    requester_laboratory_name: String,
    shared_secret: String,
    tls_certificate_sha256: Option<String>,
}

#[derive(Serialize)]
struct AcceptPairingResponse {
    node_id: Uuid,
    public_base_url: String,
    laboratory_id: Uuid,
    laboratory_name: String,
    tls_certificate_sha256: Option<String>,
    key_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct InboundPath {
    laboratory_id: Uuid,
    tail: Option<String>,
}

#[tracing::instrument(name = "Accept federation pairing", skip(pool, settings, body))]
pub async fn accept_pairing(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    body: web::Json<AcceptPairingBody>,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let payload = body.into_inner();
    let requester_base_url = normalize_base_url(&payload.requester_base_url, &settings)?;
    if payload.shared_secret.trim().is_empty() {
        return Err(FederationError::ValidationError(
            "shared_secret is required".into(),
        ));
    }
    let shared_secret_hash = sha256_hex(payload.shared_secret.as_bytes());
    let code_hash = sha256_hex(payload.pairing_code.as_bytes());

    let local_node = fetch_local_node(&pool).await?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let pairing_code = consume_pairing_code(&mut transaction, &code_hash).await?;
    let laboratory = fetch_laboratory_identity(&pool, pairing_code.local_laboratory_id).await?;
    let remote = upsert_remote_node(
        &mut transaction,
        payload.requester_node_id,
        &requester_base_url,
        Some(&payload.requester_laboratory_name),
        &payload.shared_secret,
        &shared_secret_hash,
        None,
        1,
    )
    .await?;
    upsert_trust(
        &mut transaction,
        pairing_code.local_laboratory_id,
        remote.remote_node_id,
        payload.requester_laboratory_id,
        Some(&payload.requester_laboratory_name),
        None,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(AcceptPairingResponse {
        node_id: local_node.node_id,
        public_base_url: local_node.public_base_url,
        laboratory_id: laboratory.laboratory_id,
        laboratory_name: laboratory.name,
        tls_certificate_sha256: payload.tls_certificate_sha256,
        key_version: 1,
    }))
}

#[tracing::instrument(
    name = "Federation inbound GET",
    skip(pool, settings, storage, req),
    fields(laboratory_id=tracing::field::Empty, tail=tracing::field::Empty)
)]
pub async fn inbound_get(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    storage: web::Data<AttachmentStorage>,
    path: web::Path<InboundPath>,
    req: HttpRequest,
) -> Result<HttpResponse, FederationError> {
    let path = path.into_inner();
    let laboratory_id = path.laboratory_id;
    let tail = path.tail.unwrap_or_default();
    tracing::Span::current().record("laboratory_id", tracing::field::display(laboratory_id));
    tracing::Span::current().record("tail", tracing::field::display(&tail));
    let target = parse_read_target(&tail)?;
    let context = verify_inbound_request(&req, &pool, &settings, laboratory_id).await?;
    upsert_guest_link(&pool, laboratory_id, &context).await?;
    respond_public_data(&pool, &storage, laboratory_id, target, req.query_string()).await
}

async fn consume_pairing_code(
    transaction: &mut Transaction<'_, Postgres>,
    code_hash: &str,
) -> Result<PairingCodeRow, FederationError> {
    let row = sqlx::query_as::<_, PairingCodeRow>(
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    row.ok_or_else(|| FederationError::Unauthorized("Pairing code is invalid or expired".into()))
}

async fn upsert_guest_link(
    pool: &PgPool,
    local_laboratory_id: Uuid,
    context: &super::security::InboundFederationContext,
) -> Result<Uuid, FederationError> {
    let existing: Option<Uuid> = sqlx::query_scalar(
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
    .bind(context.remote_node.remote_node_id)
    .bind(context.remote_laboratory_id)
    .bind(context.remote_user_id)
    .bind(&context.remote_username)
    .bind(&context.remote_user_type)
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    if let Some(user_id) = existing {
        return Ok(user_id);
    }

    let password_hash = hash_password(Secret::new(super::security::generate_token(32)))
        .await
        .map_err(FederationError::UnexpectedError)?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let guest_user_id = insert_shadow_guest(
        &mut transaction,
        local_laboratory_id,
        context,
        password_hash.expose_secret(),
    )
    .await?;
    let link_user_id = sqlx::query_scalar::<_, Uuid>(
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
        RETURNING local_guest_user_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(local_laboratory_id)
    .bind(context.remote_node.remote_node_id)
    .bind(context.remote_laboratory_id)
    .bind(context.remote_user_id)
    .bind(&context.remote_username)
    .bind(&context.remote_user_type)
    .bind(guest_user_id)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    if link_user_id != guest_user_id {
        sqlx::query(
            r#"
            DELETE FROM users
            WHERE user_id = $1
              AND is_federation_shadow = true
            "#,
        )
        .bind(guest_user_id)
        .execute(transaction.as_mut())
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    }
    transaction
        .commit()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    Ok(link_user_id)
}

async fn insert_shadow_guest(
    transaction: &mut Transaction<'_, Postgres>,
    local_laboratory_id: Uuid,
    context: &super::security::InboundFederationContext,
    password_hash: &str,
) -> Result<Uuid, FederationError> {
    let user_id = Uuid::new_v4();
    let username = shadow_guest_username(context);
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))
}

fn shadow_guest_username(context: &super::security::InboundFederationContext) -> String {
    let node = context.remote_node.remote_node_id.to_string();
    let user = context.remote_user_id.to_string();
    format!("fed_{}_{}", &node[..8], &user[..8])
}
