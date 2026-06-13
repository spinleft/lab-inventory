use super::helpers::fetch_laboratory;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(
    name = "Get a laboratory",
    skip(pool),
    fields(user_id=%user_id, laboratory_id=%laboratory_id)
)]
pub async fn get_laboratory(
    user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    if !actor.is_owner() && !(actor.is_maintainer() && actor.laboratory_id == Some(*laboratory_id))
    {
        return Err(ApiError::Forbidden);
    }

    let laboratory = fetch_laboratory(pool.get_ref(), *laboratory_id).await?;
    Ok(HttpResponse::Ok().json(laboratory))
}
