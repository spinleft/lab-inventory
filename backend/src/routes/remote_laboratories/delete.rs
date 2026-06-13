use super::helpers::ensure_admin;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn delete_remote_laboratory(
    user_id: UserId,
    pool: web::Data<PgPool>,
    remote_laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    ensure_admin(&actor)?;
    let result = sqlx::query("DELETE FROM remote_laboratories WHERE remote_laboratory_id = $1")
        .bind(remote_laboratory_id.into_inner())
        .execute(pool.get_ref())
        .await
        .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }

    Ok(HttpResponse::NoContent().finish())
}
