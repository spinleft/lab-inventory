use super::model::Location;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

#[tracing::instrument(name = "List locations", skip(pool), fields(user_id=%user_id))]
pub async fn list_locations(
    user_id: UserId,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let _actor = get_actor(pool.get_ref(), user_id).await?;
    let locations = sqlx::query_as::<_, Location>(
        r#"
        SELECT
            locations.location_id,
            locations.laboratory_id,
            laboratories.name AS laboratory_name,
            locations.parent_location_id,
            locations.name,
            locations.description,
            locations.is_active,
            locations.created_at,
            locations.updated_at
        FROM locations
        INNER JOIN laboratories USING (laboratory_id)
        ORDER BY laboratories.name, locations.name
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(locations))
}
