use super::model::Laboratory;
use crate::authentication::UserId;
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

#[tracing::instrument(name = "List laboratories", skip(pool), fields(user_id=%_user_id))]
pub async fn list_laboratories(
    _user_id: UserId,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let laboratories = sqlx::query_as::<_, Laboratory>(
        r#"
        SELECT laboratory_id, name, address, description, contact, created_at, updated_at
        FROM laboratories
        ORDER BY name
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(laboratories))
}
