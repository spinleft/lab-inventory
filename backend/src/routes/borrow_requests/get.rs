use super::helpers::{ensure_can_view, fetch_borrow_request};
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Get a borrow request", skip(pool), fields(user_id=%user_id, borrow_request_id=%borrow_request_id))]
pub async fn get_borrow_request(
    user_id: UserId,
    pool: web::Data<PgPool>,
    borrow_request_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let borrow_request =
        fetch_borrow_request(pool.get_ref(), borrow_request_id.into_inner()).await?;
    ensure_can_view(&actor, &borrow_request)?;
    Ok(HttpResponse::Ok().json(borrow_request))
}
