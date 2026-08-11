use super::model::{
    FederationError, GuestLinkResponse, TrustResponse, guest_link_audit_details,
    trust_audit_details,
};
use super::queries::{
    fetch_guest_link, fetch_guest_links, fetch_laboratory_identity, fetch_local_node, fetch_trusts,
    insert_pairing_code, revoke_trust_in_database, upsert_remote_node, upsert_trust,
};
use super::security::{
    ensure_enabled, generate_token, normalize_base_url, sha256_hex, validate_tls_pin_value,
    verify_tls_pin,
};
use super::service::{
    federation_reader_for_laboratory, lab_admin_for_laboratory, merge_guest_link_user,
    validate_target_guest,
};
use crate::access_control::LaboratoryContext;
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::configuration::FederationSettings;
use crate::domain::UserId;
use actix_web::{HttpRequest, HttpResponse, web};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
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
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_context)
)]
pub async fn create_pairing_code(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_context: LaboratoryContext,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    let actor = lab_admin_for_laboratory(&pool, actor_user_id, laboratory_id).await?;
    let local_node = fetch_local_node(&pool).await?;
    let code = generate_token(24);
    let code_hash = sha256_hex(code.as_bytes());
    let expires_at = Utc::now() + Duration::minutes(15);
    let row =
        insert_pairing_code(&pool, laboratory_id, &code_hash, expires_at, *actor.user_id).await?;

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
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_context)
)]
pub async fn create_trust(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    client: web::Data<reqwest::Client>,
    laboratory_context: LaboratoryContext,
    body: web::Json<CreateTrustBody>,
    req: HttpRequest,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
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
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_context)
)]
pub async fn list_trusts(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_context: LaboratoryContext,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    federation_reader_for_laboratory(&pool, actor_user_id, laboratory_id).await?;
    let trusts = fetch_trusts(&pool, laboratory_id).await?;

    Ok(HttpResponse::Ok().json(trusts))
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
    laboratory_context: LaboratoryContext,
    trust_id: web::Path<Uuid>,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    let trust_id = trust_id.into_inner();
    tracing::Span::current().record("laboratory_id", tracing::field::display(laboratory_id));
    tracing::Span::current().record("trust_id", tracing::field::display(trust_id));
    let actor = lab_admin_for_laboratory(&pool, actor_user_id, laboratory_id).await?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    let trust = revoke_trust_in_database(&mut transaction, laboratory_id, trust_id).await?;
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
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_context)
)]
pub async fn list_guest_links(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_context: LaboratoryContext,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
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
    laboratory_context: LaboratoryContext,
    link_id: web::Path<Uuid>,
    body: web::Json<MergeGuestLinkBody>,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    let link_id = link_id.into_inner();
    tracing::Span::current().record("laboratory_id", tracing::field::display(laboratory_id));
    tracing::Span::current().record("link_id", tracing::field::display(link_id));
    let actor = lab_admin_for_laboratory(&pool, actor_user_id, laboratory_id).await?;
    let target_guest_user_id = body.target_guest_user_id;
    validate_target_guest(&pool, laboratory_id, target_guest_user_id).await?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
    merge_guest_link_user(
        &mut transaction,
        laboratory_id,
        link_id,
        target_guest_user_id,
    )
    .await?;
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

/// The address the remote node should call back on.
///
/// A configured public base URL is used as-is, except in local development where
/// it points at loopback and only the address the request actually arrived on
/// can be reached from outside.
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
