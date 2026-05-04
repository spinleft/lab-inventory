use super::model::AssetCategory;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

#[tracing::instrument(name = "List asset categories", skip(pool), fields(user_id=%user_id))]
pub async fn list_asset_categories(
    user_id: UserId,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let _actor = get_actor(pool.get_ref(), user_id).await?;
    let categories = sqlx::query_as::<_, AssetCategory>(
        r#"
        SELECT
            asset_categories.category_id,
            asset_categories.laboratory_id,
            laboratories.name AS laboratory_name,
            asset_categories.name,
            asset_categories.description,
            asset_categories.created_at,
            asset_categories.updated_at
        FROM asset_categories
        INNER JOIN laboratories USING (laboratory_id)
        ORDER BY laboratories.name, asset_categories.name
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(categories))
}
