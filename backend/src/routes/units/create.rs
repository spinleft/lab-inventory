use super::model::{UnitResponse, create_unit_rollback_details};
use super::queries::{UnitDatabaseError, insert_unit};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{NewUnit, UnitCode, UnitDimension, UnitName, UnitSymbol};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    code: String,
    name: String,
    symbol: String,
    dimension: String,
    scale_to_base: f64,
    allow_decimal: bool,
}

impl TryFrom<JsonData> for NewUnit {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        Self::new(
            UnitCode::parse(value.code)?,
            UnitName::parse(value.name)?,
            UnitSymbol::parse(value.symbol)?,
            UnitDimension::parse(&value.dimension)?,
            value.scale_to_base,
            value.allow_decimal,
        )
    }
}

#[derive(thiserror::Error)]
pub enum CreateUnitError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreateUnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateUnitError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateUnitError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateUnitError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateUnitError::ConflictError(_) => StatusCode::CONFLICT,
            CreateUnitError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<UnitDatabaseError> for CreateUnitError {
    fn from(error: UnitDatabaseError) -> Self {
        match error {
            UnitDatabaseError::Validation(message) => Self::ValidationError(message),
            UnitDatabaseError::Conflict(message) => Self::ConflictError(message),
            UnitDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Create a unit",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, unit_code=%payload.code)
)]
pub async fn create_unit(
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateUnitError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    if !validate_permission(
        &pool,
        actor,
        ResourceType::Unit,
        Action::Create(*laboratory_id),
    )
    .await?
    {
        return Err(CreateUnitError::Forbidden(
            "You don't have permission to create units.".into(),
        ));
    }

    let new_unit =
        NewUnit::try_from(payload.into_inner()).map_err(CreateUnitError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let unit = insert_unit(&mut transaction, *laboratory_id, &new_unit).await?;
    record_audit(
        &mut transaction,
        actor,
        AuditAction::Create,
        AuditResource::Unit,
        Some(unit.unit_id),
        create_unit_rollback_details(&unit),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new unit.")?;

    Ok(HttpResponse::Created().json(UnitResponse::from(unit)))
}
