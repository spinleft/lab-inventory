use super::model::{UnitResponse, UnitRow};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::domain::{LaboratoryId, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum ListUnitsError {
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListUnitsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListUnitsError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListUnitsError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListUnitsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(name = "List units", skip(pool), fields(actor_user_id=%actor_user_id))]
pub async fn list_units(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
) -> Result<HttpResponse, ListUnitsError> {
    let laboratory_id: LaboratoryId = laboratory_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::Unit,
        Action::Browse(laboratory_id.into()),
    )
    .await?
    {
        return Err(ListUnitsError::Forbidden(
            "You don't have permission to view units.".into(),
        ));
    }

    let units: Vec<_> = fetch_units(&pool, laboratory_id)
        .await?
        .into_iter()
        .map(UnitResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(units))
}

async fn fetch_units(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
) -> Result<Vec<UnitRow>, ListUnitsError> {
    sqlx::query_as!(
        UnitRow,
        r#"
        SELECT unit_id, laboratory_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        FROM units
        WHERE laboratory_id = $1
        ORDER BY dimension, code
        "#,
        Uuid::from(laboratory_id),
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ListUnitsError::UnexpectedError(e.into()))
}
