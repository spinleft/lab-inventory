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
    remote_laboratory_id: Uuid,
    name: String,
    api_base_url: String,
    is_enabled: Option<bool>,
    key_id: String,
    shared_secret: String,
}

pub async fn create_remote_laboratory(
    user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    ensure_admin(&actor)?;
    let name = required_text(&payload.name, "name")?;
    let key_id = required_text(&payload.key_id, "key_id")?;
    let shared_secret = required_text(&payload.shared_secret, "shared_secret")?;
    let api_base_url = normalize_api_base_url(&payload.api_base_url)?;

    let remote = sqlx::query_as::<_, RemoteLaboratory>(
        r#"
        INSERT INTO remote_laboratories (
            remote_laboratory_id,
            name,
            api_base_url,
            is_enabled,
            key_id,
            shared_secret
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING remote_laboratory_id, name, api_base_url, is_enabled, key_id, last_seen_at, created_at, updated_at
        "#,
    )
    .bind(payload.remote_laboratory_id)
    .bind(name)
    .bind(api_base_url)
    .bind(payload.is_enabled.unwrap_or(true))
    .bind(key_id)
    .bind(shared_secret)
    .fetch_one(pool.get_ref())
    .await
    .map_err(map_database_error)?;

    Ok(HttpResponse::Created().json(remote))
}

fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ApiError::BadRequest(format!("{field} is required")))
    } else {
        Ok(trimmed)
    }
}

fn map_database_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error
        && let Some("23505") = database_error.code().as_deref()
    {
        return ApiError::Conflict("Remote laboratory already exists".into());
    }
    ApiError::UnexpectedError(error.into())
}
