use super::model::{FederationError, fetch_active_trust, fetch_local_node, fetch_remote_node};
use super::public_data::parse_read_target;
use super::security::{OutboundFederationIdentity, ensure_enabled, signed_headers, verify_tls_pin};
use crate::access_control::get_actor;
use crate::configuration::FederationSettings;
use crate::domain::{UserId, UserType};
use actix_web::http::StatusCode;
use actix_web::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use actix_web::{HttpRequest, HttpResponse, web};
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

#[derive(sqlx::FromRow)]
struct ProxyUserRow {
    user_id: Uuid,
    username: String,
    user_type_name: String,
    laboratory_id: Option<Uuid>,
}

#[tracing::instrument(
    name = "Proxy federation GET",
    skip(pool, settings, client, req),
    fields(actor_user_id=%actor_user_id)
)]
pub async fn proxy_get(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    client: web::Data<reqwest::Client>,
    path: web::Path<ProxyPath>,
    req: HttpRequest,
) -> Result<HttpResponse, FederationError> {
    ensure_enabled(&settings)?;
    let path = path.into_inner();
    let tail = path.tail.unwrap_or_default();
    parse_read_target(&tail)?;
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(FederationError::UnexpectedError)?
        .ok_or_else(|| FederationError::Forbidden("Actor not found in the database".into()))?;
    if !(actor.is_lab_admin() || actor.is_regular_user()) {
        return Err(FederationError::Forbidden(
            "Only laboratory administrators and users can use federation".into(),
        ));
    }
    let Some(local_laboratory_id) = actor.laboratory_id.map(Uuid::from) else {
        return Err(FederationError::Forbidden(
            "Federation requires a laboratory-scoped user".into(),
        ));
    };
    fetch_active_trust(
        &pool,
        local_laboratory_id,
        path.remote_node_id,
        path.remote_laboratory_id,
    )
    .await?;
    let remote_node = fetch_remote_node(&pool, path.remote_node_id).await?;
    if remote_node.status != "active" {
        return Err(FederationError::Forbidden(
            "Remote federation node is not active".into(),
        ));
    }
    let user = fetch_proxy_user(&pool, *actor.user_id).await?;
    let user_type = UserType::parse(&user.user_type_name)
        .map_err(|e| FederationError::UnexpectedError(anyhow::anyhow!(e)))?;
    if !matches!(user_type, UserType::LabAdmin | UserType::User)
        || user.laboratory_id != Some(local_laboratory_id)
    {
        return Err(FederationError::Forbidden(
            "Current user is not allowed to use federation".into(),
        ));
    }
    let local_node = fetch_local_node(&pool).await?;
    let remote_url = build_remote_url(
        &remote_node.base_url,
        path.remote_laboratory_id,
        &tail,
        req.query_string(),
    )?;
    let path_and_query = path_and_query(&remote_url);
    let identity = OutboundFederationIdentity {
        local_node_id: local_node.node_id,
        local_laboratory_id,
        user_id: user.user_id,
        username: user.username,
        user_type: user.user_type_name,
    };
    let mut request = client.get(remote_url.clone());
    for (name, value) in signed_headers("GET", &path_and_query, &[], &remote_node, &identity) {
        request = request.header(name, value);
    }
    let response = request.send().await.map_err(|e| {
        FederationError::BadGateway(format!("Remote federation request failed: {e}"))
    })?;
    verify_tls_pin(&response, remote_node.tls_certificate_sha256.as_deref())?;
    relay_response(response).await
}

async fn fetch_proxy_user(pool: &PgPool, user_id: Uuid) -> Result<ProxyUserRow, FederationError> {
    sqlx::query_as::<_, ProxyUserRow>(
        r#"
        SELECT users.user_id, users.username, user_types.name AS user_type_name, users.laboratory_id
        FROM users
        JOIN user_types USING (user_type_id)
        WHERE users.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FederationError::UnexpectedError(e.into()))?
    .ok_or_else(|| FederationError::Forbidden("Current user not found".into()))
}

fn build_remote_url(
    base_url: &str,
    remote_laboratory_id: Uuid,
    tail: &str,
    query_string: &str,
) -> Result<Url, FederationError> {
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
    Url::parse(&url)
        .map_err(|_| FederationError::UnexpectedError(anyhow::anyhow!("Invalid remote URL")))
}

fn path_and_query(url: &Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{}", url.path(), query),
        None => url.path().to_string(),
    }
}

async fn relay_response(response: reqwest::Response) -> Result<HttpResponse, FederationError> {
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|e| FederationError::BadGateway(format!("Invalid remote status: {e}")))?;
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
    let bytes = response
        .bytes()
        .await
        .map_err(|e| FederationError::BadGateway(format!("Failed to read remote response: {e}")))?;
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
