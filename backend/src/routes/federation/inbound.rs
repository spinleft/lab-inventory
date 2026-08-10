use super::model::FederationError;
use super::public_data::{parse_read_target, respond_public_data};
use super::queries::{
    consume_pairing_code, fetch_laboratory_identity, fetch_local_node, upsert_remote_node,
    upsert_trust,
};
use super::security::{ensure_enabled, normalize_base_url, sha256_hex, verify_inbound_request};
use super::service::upsert_guest_link;
use crate::configuration::FederationSettings;
use crate::file_storage::FileStorage;
use actix_web::{HttpRequest, HttpResponse, web};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
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
    storage: web::Data<FileStorage>,
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
