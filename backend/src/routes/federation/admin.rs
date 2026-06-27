use super::model::{
    FederationError, GuestLinkResponse, GuestLinkRow, PairingCodeRow, TrustResponse, TrustRow,
    fetch_laboratory_identity, fetch_local_node, guest_link_audit_details, trust_audit_details,
    upsert_remote_node, upsert_trust,
};
use super::security::{
    ensure_enabled, generate_token, normalize_base_url, sha256_hex, validate_tls_pin_value,
    verify_tls_pin,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::configuration::FederationSettings;
use crate::domain::{LaboratoryId, UserId, UserType};
use actix_web::{HttpRequest, HttpResponse, web};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Serialize)]
struct PairingCodeResponse {
    pairing_code_id: Uuid,
    pairing_code: String,
    expires_at: chrono::DateTime<Utc>,
    local_node_id: Uuid,
    local_base_url: String,
    local_laboratory_id: Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTrustBody {
    remote_base_url: String,
    remote_laboratory_id: Uuid,
    pairing_code: String,
    tls_certificate_sha256: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct AcceptPairingBody {
    pairing_code: String,
    requester_node_id: Uuid,
    requester_base_url: String,
    requester_laboratory_id: Uuid,
    requester_laboratory_name: String,
    shared_secret: String,
    tls_certificate_sha256: Option<String>,
}

#[derive(Deserialize)]
struct AcceptPairingResponse {
    node_id: Uuid,
    public_base_url: String,
    laboratory_id: Uuid,
    laboratory_name: String,
    tls_certificate_sha256: Option<String>,
    key_version: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeGuestLinkBody {
    target_guest_user_id: Uuid,
}

#[tracing::instrument(
    name = "Create federation pairing code",
    skip(pool, settings),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn create_pairing_code(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let laboratory_id = laboratory_id.into_inner();
    let actor = lab_admin_for_laboratory(&pool, actor_user_id, laboratory_id).await?;
    let local_node = fetch_local_node(&pool).await?;
    let code = generate_token(24);
    let code_hash = sha256_hex(code.as_bytes());
    let expires_at = Utc::now() + Duration::minutes(15);
    let row = sqlx::query_as::<_, PairingCodeRow>(
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
    .bind(*actor.user_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(PairingCodeResponse {
        pairing_code_id: row.pairing_code_id,
        pairing_code: code,
        expires_at: row.expires_at,
        local_node_id: local_node.node_id,
        local_base_url: local_node.public_base_url,
        local_laboratory_id: row.local_laboratory_id,
    }))
}

#[tracing::instrument(
    name = "Create federation trust",
    skip(pool, settings, client, body, req),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn create_trust(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    client: web::Data<reqwest::Client>,
    laboratory_id: web::Path<Uuid>,
    body: web::Json<CreateTrustBody>,
    req: HttpRequest,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let laboratory_id = laboratory_id.into_inner();
    let actor = lab_admin_for_laboratory(&pool, actor_user_id, laboratory_id).await?;
    let payload = body.into_inner();
    validate_tls_pin_value(payload.tls_certificate_sha256.as_deref())?;
    let remote_base_url = normalize_base_url(&payload.remote_base_url, &settings)?;
    let local_node = fetch_local_node(&pool).await?;
    let local_laboratory = fetch_laboratory_identity(&pool, laboratory_id).await?;
    let requester_base_url = requester_base_url(&req, &settings);
    let shared_secret = generate_token(32);
    let shared_secret_hash = sha256_hex(shared_secret.as_bytes());

    let accept_url = format!("{remote_base_url}/api/v1/federation/inbound/pairing/accept");
    let response = client
        .post(&accept_url)
        .json(&AcceptPairingBody {
            pairing_code: payload.pairing_code,
            requester_node_id: local_node.node_id,
            requester_base_url,
            requester_laboratory_id: laboratory_id,
            requester_laboratory_name: local_laboratory.name,
            shared_secret: shared_secret.clone(),
            tls_certificate_sha256: payload.tls_certificate_sha256.clone(),
        })
        .send()
        .await
        .map_err(|e| FederationError::BadGateway(format!("Failed to contact remote node: {e}")))?;
    verify_tls_pin(&response, payload.tls_certificate_sha256.as_deref())?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(FederationError::BadGateway(format!(
            "Remote pairing failed with status {}: {}",
            status.as_u16(),
            body
        )));
    }
    let accepted: AcceptPairingResponse = response.json().await.map_err(|e| {
        FederationError::BadGateway(format!("Invalid remote pairing response: {e}"))
    })?;
    if accepted.laboratory_id != payload.remote_laboratory_id {
        return Err(FederationError::BadGateway(
            "Remote pairing response laboratory does not match request".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let remote = upsert_remote_node(
        &mut transaction,
        accepted.node_id,
        &remote_base_url,
        Some(&accepted.public_base_url),
        &shared_secret,
        &shared_secret_hash,
        accepted
            .tls_certificate_sha256
            .as_deref()
            .or(payload.tls_certificate_sha256.as_deref()),
        accepted.key_version,
    )
    .await?;
    let trust = upsert_trust(
        &mut transaction,
        laboratory_id,
        remote.remote_node_id,
        payload.remote_laboratory_id,
        Some(&accepted.laboratory_name),
        Some(*actor.user_id),
    )
    .await?;
    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Create,
        AuditResource::FederationTrust,
        Some(trust.trust_id),
        trust_audit_details(&trust),
    )
    .await
    .map_err(FederationError::UnexpectedError)?;
    transaction
        .commit()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Created().json(TrustResponse::from_parts(trust, remote)))
}

#[tracing::instrument(
    name = "List federation trusts",
    skip(pool, settings),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn list_trusts(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let laboratory_id = laboratory_id.into_inner();
    federation_reader_for_laboratory(&pool, actor_user_id, laboratory_id).await?;
    let rows = sqlx::query_as::<_, TrustWithRemoteRow>(
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
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    Ok(HttpResponse::Ok().json(rows))
}

#[tracing::instrument(
    name = "Revoke federation trust",
    skip(pool, settings),
    fields(actor_user_id=%actor_user_id, laboratory_id=tracing::field::Empty, trust_id=tracing::field::Empty)
)]
pub async fn revoke_trust(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let (laboratory_id, trust_id) = path.into_inner();
    tracing::Span::current().record("laboratory_id", tracing::field::display(laboratory_id));
    tracing::Span::current().record("trust_id", tracing::field::display(trust_id));
    let actor = lab_admin_for_laboratory(&pool, actor_user_id, laboratory_id).await?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let trust = sqlx::query_as::<_, TrustRow>(
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Federation trust not found".into()))?;
    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Delete,
        AuditResource::FederationTrust,
        Some(trust.trust_id),
        trust_audit_details(&trust),
    )
    .await
    .map_err(FederationError::UnexpectedError)?;
    transaction
        .commit()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    Ok(HttpResponse::NoContent().finish())
}

#[tracing::instrument(
    name = "List federation guest links",
    skip(pool, settings),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn list_guest_links(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let laboratory_id = laboratory_id.into_inner();
    lab_admin_for_laboratory(&pool, actor_user_id, laboratory_id).await?;
    let links = fetch_guest_links(&pool, laboratory_id).await?;
    Ok(HttpResponse::Ok().json(
        links
            .into_iter()
            .map(GuestLinkResponse::from)
            .collect::<Vec<_>>(),
    ))
}

#[tracing::instrument(
    name = "Merge federation guest link",
    skip(pool, settings, body),
    fields(actor_user_id=%actor_user_id, laboratory_id=tracing::field::Empty, link_id=tracing::field::Empty)
)]
pub async fn merge_guest_link(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<MergeGuestLinkBody>,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let (laboratory_id, link_id) = path.into_inner();
    tracing::Span::current().record("laboratory_id", tracing::field::display(laboratory_id));
    tracing::Span::current().record("link_id", tracing::field::display(link_id));
    let actor = lab_admin_for_laboratory(&pool, actor_user_id, laboratory_id).await?;
    let target_guest_user_id = body.target_guest_user_id;
    validate_target_guest(&pool, laboratory_id, target_guest_user_id).await?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let old_guest_user_id: Uuid = sqlx::query_scalar(
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Federation guest link not found".into()))?;
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    delete_unused_shadow_guest(&mut transaction, old_guest_user_id).await?;
    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Update,
        AuditResource::FederationGuestLink,
        Some(link_id),
        guest_link_audit_details(link_id, target_guest_user_id),
    )
    .await
    .map_err(FederationError::UnexpectedError)?;
    transaction
        .commit()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;

    let link = fetch_guest_link(&pool, laboratory_id, link_id).await?;
    Ok(HttpResponse::Ok().json(GuestLinkResponse::from(link)))
}

#[derive(Serialize, sqlx::FromRow)]
struct TrustWithRemoteRow {
    trust_id: Uuid,
    local_laboratory_id: Uuid,
    remote_node_id: Uuid,
    remote_base_url: String,
    remote_laboratory_id: Uuid,
    remote_laboratory_name: Option<String>,
    status: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    revoked_at: Option<chrono::DateTime<Utc>>,
}

async fn lab_admin_for_laboratory(
    pool: &PgPool,
    actor_user_id: UserId,
    laboratory_id: Uuid,
) -> Result<Actor, FederationError> {
    let actor = get_actor(pool, actor_user_id)
        .await
        .map_err(FederationError::UnexpectedError)?
        .ok_or_else(|| FederationError::Forbidden("Actor not found in the database".into()))?;
    let laboratory_id = LaboratoryId::parse(laboratory_id)
        .map_err(|e| FederationError::UnexpectedError(anyhow::anyhow!("{e}")))?;
    if actor.is_lab_admin() && actor.laboratory_id == Some(laboratory_id) {
        Ok(actor)
    } else {
        Err(FederationError::Forbidden(
            "Only this laboratory's administrator can manage federation".into(),
        ))
    }
}

async fn federation_reader_for_laboratory(
    pool: &PgPool,
    actor_user_id: UserId,
    laboratory_id: Uuid,
) -> Result<Actor, FederationError> {
    let actor = get_actor(pool, actor_user_id)
        .await
        .map_err(FederationError::UnexpectedError)?
        .ok_or_else(|| FederationError::Forbidden("Actor not found in the database".into()))?;
    let laboratory_id = LaboratoryId::parse(laboratory_id)
        .map_err(|e| FederationError::UnexpectedError(anyhow::anyhow!("{e}")))?;
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

async fn validate_target_guest(
    pool: &PgPool,
    laboratory_id: Uuid,
    target_guest_user_id: Uuid,
) -> Result<(), FederationError> {
    let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
        r#"
        SELECT user_types.name, users.laboratory_id
        FROM users
        JOIN user_types USING (user_type_id)
        WHERE users.user_id = $1
        "#,
    )
    .bind(target_guest_user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let Some((user_type, user_laboratory_id)) = row else {
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

async fn fetch_guest_links(
    pool: &PgPool,
    laboratory_id: Uuid,
) -> Result<Vec<GuestLinkRow>, FederationError> {
    sqlx::query_as::<_, GuestLinkRow>(&guest_link_select(
        "WHERE links.local_laboratory_id = $1 ORDER BY links.last_seen_at DESC, links.link_id",
    ))
    .bind(laboratory_id)
    .fetch_all(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))
}

async fn fetch_guest_link(
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
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::NotFound("Federation guest link not found".into()))
}

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

async fn delete_unused_shadow_guest(
    transaction: &mut Transaction<'_, Postgres>,
    old_guest_user_id: Uuid,
) -> Result<(), FederationError> {
    let still_used: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM federation_guest_links
            WHERE local_guest_user_id = $1
        )
        "#,
    )
    .bind(old_guest_user_id)
    .fetch_one(transaction.as_mut())
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    if still_used {
        return Ok(());
    }
    sqlx::query(
        r#"
        DELETE FROM users
        WHERE user_id = $1
          AND is_federation_shadow = true
        "#,
    )
    .bind(old_guest_user_id)
    .execute(transaction.as_mut())
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    Ok(())
}

fn requester_base_url(req: &HttpRequest, settings: &FederationSettings) -> String {
    let connection = req.connection_info();
    let request_base_url = format!("{}://{}", connection.scheme(), connection.host());
    if settings.public_base_url.contains("127.0.0.1")
        || settings.public_base_url.contains("localhost")
    {
        request_base_url
    } else {
        settings.public_base_url.clone()
    }
}
