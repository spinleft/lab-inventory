use super::model::{TrustResponse, trust_audit_details};
use super::queries::{
    FederationDatabaseError, fetch_laboratory_identity, fetch_local_node_id, upsert_remote_node,
    upsert_trust,
};
use super::security::{
    FEDERATION_DISABLED, generate_token, normalize_base_url, sha256_hex, validate_tls_pin_value,
    verify_tls_pin,
};
use super::service::MANAGE_FEDERATION_FORBIDDEN;
use super::{AcceptPairingJsonData, AcceptPairingResponse};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::configuration::FederationSettings;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpRequest, HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTrustJsonData {
    remote_base_url: String,
    remote_laboratory_id: Uuid,
    pairing_code: String,
    tls_certificate_sha256: Option<String>,
}

#[derive(thiserror::Error)]
pub enum CreateTrustError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    /// The remote node answered, but not with something this pairing can use.
    #[error("{0}")]
    BadGateway(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreateTrustError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateTrustError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateTrustError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateTrustError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateTrustError::ConflictError(_) => StatusCode::CONFLICT,
            CreateTrustError::BadGateway(_) => StatusCode::BAD_GATEWAY,
            CreateTrustError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<FederationDatabaseError> for CreateTrustError {
    fn from(error: FederationDatabaseError) -> Self {
        match error {
            FederationDatabaseError::Validation(message) => Self::ValidationError(message),
            FederationDatabaseError::Conflict(message) => Self::ConflictError(message),
            FederationDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Create federation trust",
    skip(pool, settings, client, payload, req),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn create_trust(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    client: web::Data<reqwest::Client>,
    laboratory_context: LaboratoryContext,
    payload: web::Json<CreateTrustJsonData>,
    req: HttpRequest,
) -> Result<HttpResponse, CreateTrustError> {
    if !settings.enabled {
        return Err(CreateTrustError::Forbidden(FEDERATION_DISABLED.into()));
    }
    let authorization_actor = laboratory_context.authorization_actor();
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    if !validate_permission(
        &pool,
        &authorization_actor,
        ResourceType::Federation,
        Action::Create(laboratory_id),
    )
    .await?
    {
        return Err(CreateTrustError::Forbidden(
            MANAGE_FEDERATION_FORBIDDEN.into(),
        ));
    }
    let actor = laboratory_context.actor();
    let payload = payload.into_inner();
    validate_tls_pin_value(payload.tls_certificate_sha256.as_deref())
        .map_err(CreateTrustError::ValidationError)?;
    let remote_base_url = normalize_base_url(&payload.remote_base_url, &settings)
        .map_err(CreateTrustError::ValidationError)?;
    let local_node_id = fetch_local_node_id(&pool)
        .await?
        .context("This server has no federation node identity")?;
    let local_laboratory = fetch_laboratory_identity(&pool, laboratory_id)
        .await?
        .context("The laboratory this trust is created for no longer exists")?;
    let requester_base_url = requester_base_url(&req, &settings);
    let shared_secret = generate_token(32);
    let shared_secret_hash = sha256_hex(shared_secret.as_bytes());

    let accept_url = format!("{remote_base_url}/api/v1/federation/inbound/pairing/accept");
    let response = client
        .post(&accept_url)
        .json(&AcceptPairingJsonData {
            pairing_code: payload.pairing_code,
            requester_node_id: local_node_id,
            requester_base_url,
            requester_laboratory_id: laboratory_id,
            requester_laboratory_name: local_laboratory.name,
            shared_secret: shared_secret.clone(),
            tls_certificate_sha256: payload.tls_certificate_sha256.clone(),
        })
        .send()
        .await
        .map_err(|e| CreateTrustError::BadGateway(format!("Failed to contact remote node: {e}")))?;
    verify_tls_pin(&response, payload.tls_certificate_sha256.as_deref())
        .map_err(CreateTrustError::BadGateway)?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(CreateTrustError::BadGateway(format!(
            "Remote pairing failed with status {}: {}",
            status.as_u16(),
            body
        )));
    }
    let accepted: AcceptPairingResponse = response.json().await.map_err(|e| {
        CreateTrustError::BadGateway(format!("Invalid remote pairing response: {e}"))
    })?;
    if accepted.laboratory_id != payload.remote_laboratory_id {
        return Err(CreateTrustError::BadGateway(
            "Remote pairing response laboratory does not match request".into(),
        ));
    }

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
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
        actor,
        AuditAction::Create,
        AuditResource::FederationTrust,
        Some(trust.trust_id),
        trust_audit_details(&trust),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a federation trust")?;

    Ok(HttpResponse::Created().json(TrustResponse::from_parts(trust, remote)))
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
