use super::helpers::ensure_admin;
use super::model::RemoteLaboratory;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

pub async fn list_remote_laboratories(
    user_id: UserId,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    ensure_admin(&actor)?;

    let remotes = sqlx::query_as::<_, RemoteLaboratory>(
        r#"
        SELECT remote_laboratory_id, name, api_base_url, is_enabled, key_id, last_seen_at, created_at, updated_at
        FROM remote_laboratories
        ORDER BY name
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(remotes))
}
