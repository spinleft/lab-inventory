use super::helpers::{ensure_admin, normalize_api_base_url};
use super::model::RemoteLaboratory;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct JsonData {
    name: Option<String>,
    api_base_url: Option<String>,
    is_enabled: Option<bool>,
    key_id: Option<String>,
    shared_secret: Option<String>,
}

pub async fn update_remote_laboratory(
    user_id: UserId,
    pool: web::Data<PgPool>,
    remote_laboratory_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    ensure_admin(&actor)?;
    let remote_laboratory_id = remote_laboratory_id.into_inner();
    let api_base_url = match payload.api_base_url.as_deref() {
        Some(url) => Some(normalize_api_base_url(url)?),
        None => None,
    };

    let remote = sqlx::query_as::<_, RemoteLaboratory>(
        r#"
        UPDATE remote_laboratories
        SET
            name = COALESCE($2, name),
            api_base_url = COALESCE($3, api_base_url),
            is_enabled = COALESCE($4, is_enabled),
            key_id = COALESCE($5, key_id),
            shared_secret = COALESCE($6, shared_secret),
            updated_at = now()
        WHERE remote_laboratory_id = $1
        RETURNING remote_laboratory_id, name, api_base_url, is_enabled, key_id, last_seen_at, created_at, updated_at
        "#,
    )
    .bind(remote_laboratory_id)
    .bind(payload.name.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(api_base_url)
    .bind(payload.is_enabled)
    .bind(payload.key_id.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(payload.shared_secret.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::NotFound)?;

    Ok(HttpResponse::Ok().json(remote))
}
