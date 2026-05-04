use super::helpers::fetch_asset_category;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Get an asset category", skip(pool), fields(user_id=%user_id, category_id=%category_id))]
pub async fn get_asset_category(
    user_id: UserId,
    pool: web::Data<PgPool>,
    category_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let _actor = get_actor(pool.get_ref(), user_id).await?;
    let category = fetch_asset_category(pool.get_ref(), category_id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(category))
}
