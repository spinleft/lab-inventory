use super::model::Laboratory;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

#[tracing::instrument(name = "List laboratories", skip(pool), fields(user_id=%user_id))]
pub async fn list_laboratories(
    user_id: UserId,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    if actor.is_owner() {
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

        return Ok(HttpResponse::Ok().json(laboratories));
    }

    if actor.is_maintainer() {
        let Some(laboratory_id) = actor.laboratory_id else {
            return Ok(HttpResponse::Ok().json(Vec::<Laboratory>::new()));
        };
        let laboratories = sqlx::query_as::<_, Laboratory>(
            r#"
            SELECT laboratory_id, name, address, description, contact, created_at, updated_at
            FROM laboratories
            WHERE laboratory_id = $1
            ORDER BY name
            "#,
        )
        .bind(laboratory_id)
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

        return Ok(HttpResponse::Ok().json(laboratories));
    }

    Err(ApiError::Forbidden)
}
