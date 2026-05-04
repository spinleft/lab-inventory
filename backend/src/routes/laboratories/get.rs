use super::helpers::fetch_laboratory;
use crate::authentication::UserId;
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(
    name = "Get a laboratory",
    skip(pool),
    fields(user_id=%_user_id, laboratory_id=%laboratory_id)
)]
pub async fn get_laboratory(
    _user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let laboratory = fetch_laboratory(pool.get_ref(), *laboratory_id).await?;
    Ok(HttpResponse::Ok().json(laboratory))
}
