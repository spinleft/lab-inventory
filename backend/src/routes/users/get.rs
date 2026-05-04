use super::model::{UserResponse, fetch_user};
use super::validation::ensure_can_view_user;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(
    name = "Get a user",
    skip(pool),
    fields(actor_user_id=%user_id, target_user_id=%target_user_id)
)]
pub async fn get_user(
    user_id: UserId,
    pool: web::Data<PgPool>,
    target_user_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let target = fetch_user(pool.get_ref(), *target_user_id).await?;
    ensure_can_view_user(&actor, &target)?;

    Ok(HttpResponse::Ok().json(UserResponse::from(target)))
}
