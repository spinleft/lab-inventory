use super::helpers::fetch_asset;
use super::model::AssetResponse;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Get an asset", skip(pool), fields(user_id=%user_id, asset_id=%asset_id))]
pub async fn get_asset(
    user_id: UserId,
    pool: web::Data<PgPool>,
    asset_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let asset = fetch_asset(pool.get_ref(), asset_id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(AssetResponse::from_row(asset, &actor)))
}
