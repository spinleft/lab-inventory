use super::model::{USER_SELECT, UserResponse, UserRow};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

#[tracing::instrument(name = "List users", skip(pool), fields(user_id=%user_id))]
pub async fn list_users(
    user_id: UserId,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;

    let users = if actor.is_owner() {
        sqlx::query_as::<_, UserRow>(USER_SELECT)
            .fetch_all(pool.get_ref())
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?
    } else if actor.is_maintainer() {
        sqlx::query_as::<_, UserRow>(&format!("{USER_SELECT} WHERE users.laboratory_id = $1"))
            .bind(actor.laboratory_id)
            .fetch_all(pool.get_ref())
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?
    } else {
        sqlx::query_as::<_, UserRow>(&format!("{USER_SELECT} WHERE users.user_id = $1"))
            .bind(actor.user_id)
            .fetch_all(pool.get_ref())
            .await
            .map_err(|e| ApiError::UnexpectedError(e.into()))?
    };

    let users: Vec<UserResponse> = users.into_iter().map(UserResponse::from).collect();
    Ok(HttpResponse::Ok().json(users))
}
