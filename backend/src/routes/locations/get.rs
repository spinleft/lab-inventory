use super::helpers::fetch_location;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Get a location", skip(pool), fields(user_id=%user_id, location_id=%location_id))]
pub async fn get_location(
    user_id: UserId,
    pool: web::Data<PgPool>,
    location_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let _actor = get_actor(pool.get_ref(), user_id).await?;
    let location = fetch_location(pool.get_ref(), location_id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(location))
}
