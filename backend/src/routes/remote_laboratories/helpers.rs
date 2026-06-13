use super::model::RemoteLaboratorySecret;
use crate::authentication::Actor;
use crate::utils::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

pub fn ensure_admin(actor: &Actor) -> Result<(), ApiError> {
    if actor.is_admin() {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub async fn fetch_remote_laboratory_secret(
    pool: &PgPool,
    remote_laboratory_id: Uuid,
) -> Result<RemoteLaboratorySecret, ApiError> {
    sqlx::query_as::<_, RemoteLaboratorySecret>(
        r#"
        SELECT
            api_base_url,
            is_enabled,
            key_id,
            shared_secret
        FROM remote_laboratories
        WHERE remote_laboratory_id = $1
        "#,
    )
    .bind(remote_laboratory_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?
    .ok_or(ApiError::NotFound)
}

pub fn normalize_api_base_url(input: &str) -> Result<String, ApiError> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(ApiError::BadRequest("api_base_url is required".into()));
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(ApiError::BadRequest(
            "api_base_url must use http or https".into(),
        ));
    }
    if trimmed.ends_with("/api/v1") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{trimmed}/api/v1"))
    }
}
