use super::borrowing::parse_borrow_target;
use super::model::RemoteNodeRow;
use super::public_data::{PublicDataError, parse_read_target};
use super::queries::{
    fetch_active_trust, fetch_local_node_id, fetch_proxy_user, fetch_remote_node,
};
use super::security::{
    FEDERATION_DISABLED, OutboundFederationIdentity, signed_headers, verify_tls_pin,
};
use crate::access_control::Actor;
use crate::configuration::FederationSettings;
use crate::domain::UserType;
use crate::utils::error_chain_fmt;
use actix_web::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use actix_web::http::{Method, StatusCode};
use actix_web::{HttpRequest, HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ProxyPath {
    remote_node_id: Uuid,
    remote_laboratory_id: Uuid,
    tail: Option<String>,
}

#[derive(thiserror::Error)]
pub enum ProxyFederationError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    /// The remote node could not be reached, or answered with something this
    /// server cannot relay.
    #[error("{0}")]
    BadGateway(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ProxyFederationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ProxyFederationError {
    fn status_code(&self) -> StatusCode {
        match self {
            ProxyFederationError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ProxyFederationError::Forbidden(_) => StatusCode::FORBIDDEN,
            ProxyFederationError::NotFound(_) => StatusCode::NOT_FOUND,
            ProxyFederationError::BadGateway(_) => StatusCode::BAD_GATEWAY,
            ProxyFederationError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<PublicDataError> for ProxyFederationError {
    fn from(error: PublicDataError) -> Self {
        match error {
            PublicDataError::Validation(message) => Self::ValidationError(message),
            PublicDataError::NotFound(message) => Self::NotFound(message),
            PublicDataError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Proxy federation GET",
    skip(pool, settings, client, req),
    fields(actor_user_id=%actor.user_id)
)]
pub async fn proxy_get(
    actor: Actor,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    client: web::Data<reqwest::Client>,
    path: web::Path<ProxyPath>,
    req: HttpRequest,
) -> Result<HttpResponse, ProxyFederationError> {
    let path = path.into_inner();
    let tail = path.tail.clone().unwrap_or_default();
    // Borrow reads share the tail namespace with the public reads, so the borrow
    // parser gets first refusal; anything it does not claim has to be a read.
    if parse_borrow_target(&Method::GET, &tail).is_none() {
        parse_read_target(&tail)?;
    }
    let (remote_node, identity) = authorize_proxy(&actor, &pool, &settings, &path).await?;
    let remote_url = build_remote_url(
        &remote_node.base_url,
        path.remote_laboratory_id,
        &tail,
        req.query_string(),
    )?;
    let path_and_query = path_and_query(&remote_url);
    let mut request = client.get(remote_url);
    for (name, value) in signed_headers("GET", &path_and_query, &[], &remote_node, &identity) {
        request = request.header(name, value);
    }
    let response = request.send().await.map_err(|e| {
        ProxyFederationError::BadGateway(format!("Remote federation request failed: {e}"))
    })?;
    verify_tls_pin(&response, remote_node.tls_certificate_sha256.as_deref())
        .map_err(ProxyFederationError::BadGateway)?;

    relay_response(response).await
}

/// Relays a write to a partner laboratory on behalf of the signed-in user.
///
/// Only the borrow operations are reachable: `parse_borrow_target` is the whole
/// list of what this server is willing to sign its name to.
#[tracing::instrument(
    name = "Proxy federation POST",
    skip(pool, settings, client, body, req),
    fields(actor_user_id=%actor.user_id)
)]
pub async fn proxy_post(
    actor: Actor,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    client: web::Data<reqwest::Client>,
    path: web::Path<ProxyPath>,
    body: web::Bytes,
    req: HttpRequest,
) -> Result<HttpResponse, ProxyFederationError> {
    let path = path.into_inner();
    let tail = path.tail.clone().unwrap_or_default();
    if parse_borrow_target(&Method::POST, &tail).is_none() {
        return Err(ProxyFederationError::NotFound(
            "Federation route not found".into(),
        ));
    }
    let (remote_node, identity) = authorize_proxy(&actor, &pool, &settings, &path).await?;
    let remote_url = build_remote_url(
        &remote_node.base_url,
        path.remote_laboratory_id,
        &tail,
        req.query_string(),
    )?;
    let path_and_query = path_and_query(&remote_url);
    let mut request = client
        .post(remote_url)
        .header(reqwest::header::CONTENT_TYPE, "application/json");
    // The signature covers the hash of exactly these bytes, so they are forwarded
    // verbatim. Parsing and re-serializing would change the encoding just enough
    // to make every write fail its signature check at the far end.
    for (name, value) in signed_headers("POST", &path_and_query, &body, &remote_node, &identity) {
        request = request.header(name, value);
    }
    let response = request.body(body).send().await.map_err(|e| {
        ProxyFederationError::BadGateway(format!("Remote federation request failed: {e}"))
    })?;
    verify_tls_pin(&response, remote_node.tls_certificate_sha256.as_deref())
        .map_err(ProxyFederationError::BadGateway)?;

    relay_response(response).await
}

/// Everything that has to be true before this server signs a request as itself,
/// shared by every proxied method so the checks cannot drift apart.
///
/// The user is re-read from the database rather than taken from the session:
/// what gets signed is what the database says about them right now.
async fn authorize_proxy(
    actor: &Actor,
    pool: &PgPool,
    settings: &FederationSettings,
    path: &ProxyPath,
) -> Result<(RemoteNodeRow, OutboundFederationIdentity), ProxyFederationError> {
    if !settings.enabled {
        return Err(ProxyFederationError::Forbidden(FEDERATION_DISABLED.into()));
    }
    if !(actor.is_lab_admin() || actor.is_regular_user()) {
        return Err(ProxyFederationError::Forbidden(
            "Only laboratory administrators and users can use federation".into(),
        ));
    }
    let Some(local_laboratory_id) = actor.laboratory_id.map(Uuid::from) else {
        return Err(ProxyFederationError::Forbidden(
            "Federation requires a laboratory-scoped user".into(),
        ));
    };
    if fetch_active_trust(
        pool,
        local_laboratory_id,
        path.remote_node_id,
        path.remote_laboratory_id,
    )
    .await?
    .is_none()
    {
        return Err(ProxyFederationError::Forbidden(
            "No active federation trust between these laboratories".into(),
        ));
    }
    let remote_node = fetch_remote_node(pool, path.remote_node_id)
        .await?
        .ok_or_else(|| ProxyFederationError::NotFound("Remote federation node not found".into()))?;
    if remote_node.status != "active" {
        return Err(ProxyFederationError::Forbidden(
            "Remote federation node is not active".into(),
        ));
    }
    let user = fetch_proxy_user(pool, *actor.user_id)
        .await?
        .ok_or_else(|| ProxyFederationError::Forbidden("Current user not found".into()))?;
    let user_type = UserType::parse(&user.user_type_name)
        .map_err(|e| ProxyFederationError::UnexpectedError(anyhow::anyhow!(e)))?;
    if !matches!(user_type, UserType::LabAdmin | UserType::User)
        || user.laboratory_id != Some(local_laboratory_id)
    {
        return Err(ProxyFederationError::Forbidden(
            "Current user is not allowed to use federation".into(),
        ));
    }
    let local_node_id = fetch_local_node_id(pool)
        .await?
        .context("This server has no federation node identity")?;

    Ok((
        remote_node,
        OutboundFederationIdentity {
            local_node_id,
            local_laboratory_id,
            user_id: user.user_id,
            username: user.username,
            user_type: user.user_type_name,
        },
    ))
}

fn build_remote_url(
    base_url: &str,
    remote_laboratory_id: Uuid,
    tail: &str,
    query_string: &str,
) -> Result<Url, ProxyFederationError> {
    let mut url = format!(
        "{}/api/v1/federation/inbound/laboratories/{}",
        base_url.trim_end_matches('/'),
        remote_laboratory_id
    );
    let tail = tail.trim_matches('/');
    if !tail.is_empty() {
        url.push('/');
        url.push_str(tail);
    }
    if !query_string.is_empty() {
        url.push('?');
        url.push_str(query_string);
    }
    Ok(Url::parse(&url).context("Failed to build the remote federation URL")?)
}

fn path_and_query(url: &Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{}", url.path(), query),
        None => url.path().to_string(),
    }
}

async fn relay_response(response: reqwest::Response) -> Result<HttpResponse, ProxyFederationError> {
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|e| ProxyFederationError::BadGateway(format!("Invalid remote status: {e}")))?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let content_disposition = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let bytes = response.bytes().await.map_err(|e| {
        ProxyFederationError::BadGateway(format!("Failed to read remote response: {e}"))
    })?;
    let mut builder = HttpResponse::build(status);
    if let Some(content_type) = content_type {
        builder.insert_header((CONTENT_TYPE, content_type));
    }
    if let Some(content_disposition) = content_disposition {
        builder.insert_header((CONTENT_DISPOSITION, content_disposition));
    }
    builder.insert_header((CONTENT_LENGTH, bytes.len().to_string()));

    Ok(builder.body(bytes))
}
