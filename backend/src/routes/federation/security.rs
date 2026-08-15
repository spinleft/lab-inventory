use super::model::RemoteNodeRow;
use super::queries::{FederationDatabaseError, fetch_active_trust, fetch_remote_node};
use super::service::remember_nonce;
use crate::configuration::FederationSettings;
use crate::utils::error_chain_fmt;
use actix_web::HttpRequest;
use actix_web::http::header::HeaderMap;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use chrono::Utc;
use hmac::{Hmac, Mac};
use rand::RngCore;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::Url;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// What every route answers with while federation is switched off, so the wording
/// does not drift between them.
pub(super) const FEDERATION_DISABLED: &str = "Federation is disabled";

/// What can go wrong proving an inbound request is genuine.
///
/// These are failure modes of the protocol rather than of any one route, so they
/// are named here and the routes that speak the protocol map them onto their own
/// error. The checks that only inspect what a caller sent — a URL, a certificate
/// pin — report a plain message instead, the way the domain parsers do.
#[derive(thiserror::Error)]
pub(super) enum FederationSecurityError {
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for FederationSecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

/// The only statements issued while verifying a request are the nonce ones, so a
/// conflict here is a nonce that was already spent — a replay, not a caller
/// mistake.
impl From<FederationDatabaseError> for FederationSecurityError {
    fn from(error: FederationDatabaseError) -> Self {
        match error {
            FederationDatabaseError::Conflict(message) => Self::Unauthorized(message),
            // Nothing a signed request carries reaches these statements as data,
            // so a rejected write is this server's problem, not the caller's.
            FederationDatabaseError::Validation(message) => {
                Self::Unexpected(anyhow::anyhow!(message))
            }
            FederationDatabaseError::Unexpected(error) => Self::Unexpected(error),
        }
    }
}

const HEADER_NODE_ID: &str = "x-federation-node-id";
const HEADER_KEY_VERSION: &str = "x-federation-key-version";
const HEADER_TIMESTAMP: &str = "x-federation-timestamp";
const HEADER_NONCE: &str = "x-federation-nonce";
const HEADER_SIGNATURE: &str = "x-federation-signature";
const HEADER_REMOTE_LABORATORY_ID: &str = "x-federation-remote-laboratory-id";
const HEADER_REMOTE_USER_ID: &str = "x-federation-remote-user-id";
const HEADER_REMOTE_USERNAME: &str = "x-federation-remote-username";
const HEADER_REMOTE_USER_TYPE: &str = "x-federation-remote-user-type";

#[derive(Clone)]
pub(super) struct OutboundFederationIdentity {
    pub(super) local_node_id: Uuid,
    pub(super) local_laboratory_id: Uuid,
    pub(super) user_id: Uuid,
    pub(super) username: String,
    pub(super) user_type: String,
}

#[derive(Clone)]
pub(super) struct InboundFederationContext {
    pub(super) remote_node: RemoteNodeRow,
    pub(super) remote_laboratory_id: Uuid,
    pub(super) remote_user_id: Uuid,
    pub(super) remote_username: String,
    pub(super) remote_user_type: String,
}

pub(super) fn generate_token(byte_len: usize) -> String {
    let mut bytes = vec![0; byte_len];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn normalize_base_url(
    input: &str,
    settings: &FederationSettings,
) -> Result<String, String> {
    let url = Url::parse(input).map_err(|_| String::from("Invalid remote base URL"))?;
    validate_remote_url(&url, settings)?;
    let mut normalized = url;
    normalized.set_query(None);
    normalized.set_fragment(None);
    if normalized.path() != "/" && !normalized.path().is_empty() {
        return Err(String::from("Remote base URL cannot include a path"));
    }
    Ok(normalized.as_str().trim_end_matches('/').to_string())
}

pub(super) fn validate_tls_pin_value(value: Option<&str>) -> Result<(), String> {
    if let Some(value) = value {
        let valid = value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit());
        if !valid {
            return Err(String::from(
                "TLS certificate SHA-256 pin must be 64 hex characters",
            ));
        }
    }
    Ok(())
}

pub(super) fn signed_headers(
    method: &str,
    path_and_query: &str,
    body: &[u8],
    remote_node: &RemoteNodeRow,
    identity: &OutboundFederationIdentity,
) -> Vec<(&'static str, String)> {
    let timestamp = Utc::now().timestamp().to_string();
    let nonce = generate_token(24);
    let body_hash = sha256_hex(body);
    let signature = sign_canonical(
        &remote_node.shared_secret,
        method,
        path_and_query,
        &body_hash,
        identity.local_node_id,
        identity.local_laboratory_id,
        identity.user_id,
        &identity.user_type,
        &timestamp,
        &nonce,
        remote_node.key_version,
    );
    vec![
        (HEADER_NODE_ID, identity.local_node_id.to_string()),
        (HEADER_KEY_VERSION, remote_node.key_version.to_string()),
        (HEADER_TIMESTAMP, timestamp),
        (HEADER_NONCE, nonce),
        (HEADER_SIGNATURE, signature),
        (
            HEADER_REMOTE_LABORATORY_ID,
            identity.local_laboratory_id.to_string(),
        ),
        (HEADER_REMOTE_USER_ID, identity.user_id.to_string()),
        (HEADER_REMOTE_USERNAME, identity.username.clone()),
        (HEADER_REMOTE_USER_TYPE, identity.user_type.clone()),
    ]
}

/// `body` is the request body exactly as it arrived. The signature covers its
/// hash, so passing anything other than the received bytes — a re-serialized
/// value, or an empty slice on a request that had one — would either reject a
/// legitimate caller or, worse, accept a body nobody signed.
pub(super) async fn verify_inbound_request(
    req: &HttpRequest,
    body: &[u8],
    pool: &PgPool,
    settings: &FederationSettings,
    target_laboratory_id: Uuid,
) -> Result<InboundFederationContext, FederationSecurityError> {
    if !settings.enabled {
        return Err(FederationSecurityError::Forbidden(
            FEDERATION_DISABLED.into(),
        ));
    }
    let headers = req.headers();
    let remote_node_id = parse_uuid_header(headers, HEADER_NODE_ID)?;
    let key_version = parse_i32_header(headers, HEADER_KEY_VERSION)?;
    let timestamp = parse_i64_header(headers, HEADER_TIMESTAMP)?;
    let nonce = required_header(headers, HEADER_NONCE)?.to_string();
    let signature = required_header(headers, HEADER_SIGNATURE)?.to_string();
    let remote_laboratory_id = parse_uuid_header(headers, HEADER_REMOTE_LABORATORY_ID)?;
    let remote_user_id = parse_uuid_header(headers, HEADER_REMOTE_USER_ID)?;
    let remote_username = required_header(headers, HEADER_REMOTE_USERNAME)?
        .trim()
        .to_string();
    let remote_user_type = required_header(headers, HEADER_REMOTE_USER_TYPE)?
        .trim()
        .to_string();

    if !matches!(remote_user_type.as_str(), "lab_admin" | "user") {
        return Err(FederationSecurityError::Forbidden(
            "Remote user type is not allowed for federation".into(),
        ));
    }
    if remote_username.is_empty() {
        return Err(FederationSecurityError::Unauthorized(
            "Remote username is required".into(),
        ));
    }

    let now = Utc::now().timestamp();
    if (now - timestamp).abs() > settings.request_ttl_seconds {
        return Err(FederationSecurityError::Unauthorized(
            "Federation request timestamp is outside the allowed window".into(),
        ));
    }

    // An unknown node is answered exactly like a known one that fails to sign,
    // so probing this endpoint cannot tell the two apart.
    let remote_node = fetch_remote_node(pool, remote_node_id)
        .await?
        .ok_or_else(|| {
            FederationSecurityError::Unauthorized("Federation node is not known".into())
        })?;
    if remote_node.status != "active" || remote_node.key_version != key_version {
        return Err(FederationSecurityError::Unauthorized(
            "Federation node is not active or key version is invalid".into(),
        ));
    }
    if fetch_active_trust(
        pool,
        target_laboratory_id,
        remote_node_id,
        remote_laboratory_id,
    )
    .await?
    .is_none()
    {
        return Err(FederationSecurityError::Forbidden(
            "No active federation trust between these laboratories".into(),
        ));
    }

    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or_else(|| req.path());
    let body_hash = sha256_hex(body);
    let expected = sign_canonical(
        &remote_node.shared_secret,
        req.method().as_str(),
        path_and_query,
        &body_hash,
        remote_node_id,
        remote_laboratory_id,
        remote_user_id,
        &remote_user_type,
        &timestamp.to_string(),
        &nonce,
        key_version,
    );
    verify_signature(&signature, &expected)?;

    // Only a request that already proved its signature may spend a nonce, so an
    // unsigned caller cannot write to the nonce table on our behalf. The nonce is
    // part of the signed material, so a replay cannot carry a fresh one either.
    remember_nonce(pool, remote_node_id, &nonce, settings.request_ttl_seconds).await?;

    Ok(InboundFederationContext {
        remote_node,
        remote_laboratory_id,
        remote_user_id,
        remote_username,
        remote_user_type,
    })
}

pub(super) fn verify_tls_pin(
    response: &reqwest::Response,
    expected_pin: Option<&str>,
) -> Result<(), String> {
    let Some(expected_pin) = expected_pin else {
        return Ok(());
    };
    let Some(tls_info) = response.extensions().get::<reqwest::tls::TlsInfo>() else {
        return Err(String::from(
            "TLS peer certificate information is unavailable",
        ));
    };
    let Some(peer_certificate) = tls_info.peer_certificate() else {
        return Err(String::from("TLS peer certificate is unavailable"));
    };
    let actual_pin = sha256_hex(peer_certificate);
    if actual_pin.eq_ignore_ascii_case(expected_pin) {
        Ok(())
    } else {
        Err(String::from("Remote TLS certificate pin does not match"))
    }
}

fn sign_canonical(
    shared_secret: &str,
    method: &str,
    path_and_query: &str,
    body_hash: &str,
    node_id: Uuid,
    laboratory_id: Uuid,
    user_id: Uuid,
    user_type: &str,
    timestamp: &str,
    nonce: &str,
    key_version: i32,
) -> String {
    let canonical = format!(
        "v1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        method.to_uppercase(),
        path_and_query,
        body_hash,
        node_id,
        laboratory_id,
        user_id,
        user_type,
        timestamp,
        nonce,
        key_version,
    );
    let mut mac = HmacSha256::new_from_slice(shared_secret.as_bytes())
        .expect("HMAC accepts keys of any size");
    mac.update(canonical.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}

fn verify_signature(provided: &str, expected: &str) -> Result<(), FederationSecurityError> {
    let provided = STANDARD.decode(provided).map_err(|_| {
        FederationSecurityError::Unauthorized("Invalid federation signature".into())
    })?;
    let expected = STANDARD.decode(expected).map_err(|_| {
        FederationSecurityError::Unauthorized("Invalid federation signature".into())
    })?;
    if provided.len() == expected.len()
        && provided
            .iter()
            .zip(expected.iter())
            .fold(0u8, |acc, (left, right)| acc | (left ^ right))
            == 0
    {
        Ok(())
    } else {
        Err(FederationSecurityError::Unauthorized(
            "Federation signature mismatch".into(),
        ))
    }
}

fn required_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
) -> Result<&'a str, FederationSecurityError> {
    headers
        .get(name)
        .ok_or_else(|| {
            FederationSecurityError::Unauthorized(format!("Missing federation header: {name}"))
        })?
        .to_str()
        .map_err(|_| {
            FederationSecurityError::Unauthorized(format!("Invalid federation header: {name}"))
        })
}

fn parse_uuid_header(
    headers: &HeaderMap,
    name: &'static str,
) -> Result<Uuid, FederationSecurityError> {
    required_header(headers, name)?.parse().map_err(|_| {
        FederationSecurityError::Unauthorized(format!("Invalid federation header: {name}"))
    })
}

fn parse_i32_header(
    headers: &HeaderMap,
    name: &'static str,
) -> Result<i32, FederationSecurityError> {
    required_header(headers, name)?.parse().map_err(|_| {
        FederationSecurityError::Unauthorized(format!("Invalid federation header: {name}"))
    })
}

fn parse_i64_header(
    headers: &HeaderMap,
    name: &'static str,
) -> Result<i64, FederationSecurityError> {
    required_header(headers, name)?.parse().map_err(|_| {
        FederationSecurityError::Unauthorized(format!("Invalid federation header: {name}"))
    })
}
fn validate_remote_url(url: &Url, settings: &FederationSettings) -> Result<(), String> {
    match url.scheme() {
        "https" => {}
        "http" if !settings.require_https && settings.allow_insecure_private_network => {}
        "http" => {
            return Err(String::from(
                "HTTP federation URLs are not allowed by this configuration",
            ));
        }
        _ => {
            return Err(String::from("Federation URL must use http or https"));
        }
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(String::from("Federation URL cannot contain credentials"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| String::from("Federation URL must include a host"))?
        .to_ascii_lowercase();
    let host_with_port = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    };
    let allowlisted = settings
        .allowed_remote_hosts
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .any(|value| value == host || value == host_with_port);
    if !settings.allowed_remote_hosts.is_empty() && !allowlisted {
        return Err(String::from(
            "Remote host is not in federation allowed_remote_hosts",
        ));
    }

    if host == "localhost" {
        if settings.allow_insecure_private_network || allowlisted {
            return Ok(());
        }
        return Err(String::from("Localhost federation URLs are not allowed"));
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        validate_remote_ip(ip, settings, allowlisted)?;
    }
    if url.scheme() == "http" && !settings.allow_insecure_private_network && !allowlisted {
        return Err(String::from(
            "HTTP federation URLs require explicit private-network allowance",
        ));
    }
    Ok(())
}

fn validate_remote_ip(
    ip: IpAddr,
    settings: &FederationSettings,
    allowlisted: bool,
) -> Result<(), String> {
    if is_metadata_ip(ip) {
        return Err(String::from(
            "Metadata service IPs are not allowed as federation targets",
        ));
    }
    let special = match ip {
        IpAddr::V4(ip) => {
            ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_private()
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_multicast()
                || ip.is_unspecified()
                || is_ipv6_unique_local(ip)
                || is_ipv6_link_local(ip)
        }
    };
    if special && !(settings.allow_insecure_private_network || allowlisted) {
        return Err(String::from(
            "Private or special-use federation target IPs require explicit allowance",
        ));
    }
    Ok(())
}

fn is_metadata_ip(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V4(ip) if ip == Ipv4Addr::new(169, 254, 169, 254))
}

fn is_ipv6_unique_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_ipv6_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}
