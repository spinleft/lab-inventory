use super::model::Unit;
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;

#[tracing::instrument(name = "List units", skip(pool))]
pub async fn list_units(pool: web::Data<PgPool>) -> Result<HttpResponse, ApiError> {
    let units = sqlx::query_as::<_, Unit>(
        r#"
        SELECT unit_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        FROM units
        ORDER BY dimension, code
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| ApiError::UnexpectedError(e.into()))?;

    Ok(HttpResponse::Ok().json(units))
}
