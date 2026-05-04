use super::helpers::fetch_maintenance_record;
use super::model::MaintenanceRecordResponse;
use crate::authentication::{UserId, get_actor};
use crate::utils::ApiError;
use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(name = "Get a maintenance record", skip(pool), fields(user_id=%user_id, maintenance_record_id=%maintenance_record_id))]
pub async fn get_maintenance_record(
    user_id: UserId,
    pool: web::Data<PgPool>,
    maintenance_record_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let actor = get_actor(pool.get_ref(), user_id).await?;
    let record =
        fetch_maintenance_record(pool.get_ref(), maintenance_record_id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(MaintenanceRecordResponse::from_row(record, &actor)))
}
