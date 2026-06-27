use super::model::{FederationError, RemoteNodeRow, fetch_active_trust, fetch_remote_node};
use crate::configuration::FederationSettings;
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

pub(super) fn ensure_enabled(settings: &FederationSettings) -> Result<(), FederationError> {
    if settings.enabled {
        Ok(())
    } else {
        Err(FederationError::Disabled)
    }
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
) -> Result<String, FederationError> {
    let url = Url::parse(input)
        .map_err(|_| FederationError::ValidationError("Invalid remote base URL".into()))?;
    validate_remote_url(&url, settings)?;
    let mut normalized = url;
    normalized.set_query(None);
    normalized.set_fragment(None);
    if normalized.path() != "/" && !normalized.path().is_empty() {
        return Err(FederationError::ValidationError(
            "Remote base URL cannot include a path".into(),
        ));
    }
    Ok(normalized.as_str().trim_end_matches('/').to_string())
}

pub(super) fn validate_tls_pin_value(value: Option<&str>) -> Result<(), FederationError> {
    if let Some(value) = value {
        let valid = value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit());
        if !valid {
            return Err(FederationError::ValidationError(
                "TLS certificate SHA-256 pin must be 64 hex characters".into(),
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

pub(super) async fn verify_inbound_request(
    req: &HttpRequest,
    pool: &PgPool,
    settings: &FederationSettings,
    target_laboratory_id: Uuid,
) -> Result<InboundFederationContext, FederationError> {
    ensure_enabled(settings)?;
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
        return Err(FederationError::Forbidden(
            "Remote user type is not allowed for federation".into(),
        ));
    }
    if remote_username.is_empty() {
        return Err(FederationError::Unauthorized(
            "Remote username is required".into(),
        ));
    }

    let now = Utc::now().timestamp();
    if (now - timestamp).abs() > settings.request_ttl_seconds {
        return Err(FederationError::Unauthorized(
            "Federation request timestamp is outside the allowed window".into(),
        ));
    }

    let remote_node = fetch_remote_node(pool, remote_node_id).await?;
    if remote_node.status != "active" || remote_node.key_version != key_version {
        return Err(FederationError::Unauthorized(
            "Federation node is not active or key version is invalid".into(),
        ));
    }
    fetch_active_trust(
        pool,
        target_laboratory_id,
        remote_node_id,
        remote_laboratory_id,
    )
    .await?;
    remember_nonce(pool, remote_node_id, &nonce, settings.request_ttl_seconds).await?;

    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or_else(|| req.path());
    let body_hash = sha256_hex(&[]);
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
) -> Result<(), FederationError> {
    let Some(expected_pin) = expected_pin else {
        return Ok(());
    };
    let Some(tls_info) = response.extensions().get::<reqwest::tls::TlsInfo>() else {
        return Err(FederationError::BadGateway(
            "TLS peer certificate information is unavailable".into(),
        ));
    };
    let Some(peer_certificate) = tls_info.peer_certificate() else {
        return Err(FederationError::BadGateway(
            "TLS peer certificate is unavailable".into(),
        ));
    };
    let actual_pin = sha256_hex(peer_certificate);
    if actual_pin.eq_ignore_ascii_case(expected_pin) {
        Ok(())
    } else {
        Err(FederationError::BadGateway(
            "Remote TLS certificate pin does not match".into(),
        ))
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

fn verify_signature(provided: &str, expected: &str) -> Result<(), FederationError> {
    let provided = STANDARD
        .decode(provided)
        .map_err(|_| FederationError::Unauthorized("Invalid federation signature".into()))?;
    let expected = STANDARD
        .decode(expected)
        .map_err(|_| FederationError::Unauthorized("Invalid federation signature".into()))?;
    if provided.len() == expected.len()
        && provided
            .iter()
            .zip(expected.iter())
            .fold(0u8, |acc, (left, right)| acc | (left ^ right))
            == 0
    {
        Ok(())
    } else {
        Err(FederationError::Unauthorized(
            "Federation signature mismatch".into(),
        ))
    }
}

fn required_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
) -> Result<&'a str, FederationError> {
    headers
        .get(name)
        .ok_or_else(|| FederationError::Unauthorized(format!("Missing federation header: {name}")))?
        .to_str()
        .map_err(|_| FederationError::Unauthorized(format!("Invalid federation header: {name}")))
}

fn parse_uuid_header(headers: &HeaderMap, name: &'static str) -> Result<Uuid, FederationError> {
    required_header(headers, name)?
        .parse()
        .map_err(|_| FederationError::Unauthorized(format!("Invalid federation header: {name}")))
}

fn parse_i32_header(headers: &HeaderMap, name: &'static str) -> Result<i32, FederationError> {
    required_header(headers, name)?
        .parse()
        .map_err(|_| FederationError::Unauthorized(format!("Invalid federation header: {name}")))
}

fn parse_i64_header(headers: &HeaderMap, name: &'static str) -> Result<i64, FederationError> {
    required_header(headers, name)?
        .parse()
        .map_err(|_| FederationError::Unauthorized(format!("Invalid federation header: {name}")))
}

async fn remember_nonce(
    pool: &PgPool,
    remote_node_id: Uuid,
    nonce: &str,
    ttl_seconds: i64,
) -> Result<(), FederationError> {
    sqlx::query("DELETE FROM federation_request_nonces WHERE expires_at <= now()")
        .execute(pool)
        .await
        .map_err(|e| FederationError::UnexpectedError(e.into()))?;
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
        Err(error) => Err(FederationError::UnexpectedError(error.into())),
    }
}

fn validate_remote_url(url: &Url, settings: &FederationSettings) -> Result<(), FederationError> {
    match url.scheme() {
        "https" => {}
        "http" if !settings.require_https && settings.allow_insecure_private_network => {}
        "http" => {
            return Err(FederationError::ValidationError(
                "HTTP federation URLs are not allowed by this configuration".into(),
            ));
        }
        _ => {
            return Err(FederationError::ValidationError(
                "Federation URL must use http or https".into(),
            ));
        }
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(FederationError::ValidationError(
            "Federation URL cannot contain credentials".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| {
            FederationError::ValidationError("Federation URL must include a host".into())
        })?
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
        return Err(FederationError::ValidationError(
            "Remote host is not in federation allowed_remote_hosts".into(),
        ));
    }

    if host == "localhost" {
        if settings.allow_insecure_private_network || allowlisted {
            return Ok(());
        }
        return Err(FederationError::ValidationError(
            "Localhost federation URLs are not allowed".into(),
        ));
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        validate_remote_ip(ip, settings, allowlisted)?;
    }
    if url.scheme() == "http" && !settings.allow_insecure_private_network && !allowlisted {
        return Err(FederationError::ValidationError(
            "HTTP federation URLs require explicit private-network allowance".into(),
        ));
    }
    Ok(())
}

fn validate_remote_ip(
    ip: IpAddr,
    settings: &FederationSettings,
    allowlisted: bool,
) -> Result<(), FederationError> {
    if is_metadata_ip(ip) {
        return Err(FederationError::ValidationError(
            "Metadata service IPs are not allowed as federation targets".into(),
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
        return Err(FederationError::ValidationError(
            "Private or special-use federation target IPs require explicit allowance".into(),
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
