use super::model::{
    UnitDatabaseError, UnitResponse, UnitRow, create_unit_rollback_details, map_unit_database_error,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{NewUnit, UnitCode, UnitDimension, UnitName, UnitSymbol, UserId};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

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

#[tracing::instrument(
    name = "Create a unit",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, unit_code=%payload.code)
)]
pub async fn create_unit(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateUnitError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(CreateUnitError::UnexpectedError)?
        .ok_or(CreateUnitError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_create_permission(&actor)?;
    let new_unit =
        NewUnit::try_from(payload.into_inner()).map_err(CreateUnitError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let unit = insert_new_unit(&mut transaction, new_unit).await?;
    record_audit(
        &mut transaction,
        &actor,
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

fn validate_create_permission(actor: &Actor) -> Result<(), CreateUnitError> {
    if actor.can_manage_units() {
        Ok(())
    } else {
        Err(CreateUnitError::Forbidden(
            "You don't have permission to create units.".into(),
        ))
    }
}

#[tracing::instrument(name = "Saving new unit in the database", skip(transaction, new_unit))]
async fn insert_new_unit(
    transaction: &mut Transaction<'_, Postgres>,
    new_unit: NewUnit,
) -> Result<UnitRow, CreateUnitError> {
    let dimension = new_unit.dimension.to_string();
    sqlx::query_as!(
        UnitRow,
        r#"
        INSERT INTO units (unit_id, code, name, symbol, dimension, scale_to_base, allow_decimal)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING unit_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        "#,
        Uuid::new_v4(),
        new_unit.code.as_ref(),
        new_unit.name.as_ref(),
        new_unit.symbol.as_ref(),
        &dimension,
        new_unit.scale_to_base,
        new_unit.allow_decimal,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

fn map_database_error(error: sqlx::Error) -> CreateUnitError {
    if let Some(mapped) = map_unit_database_error(
        &error,
        "Unit code already exists",
        "Unit already exists",
        "Invalid unit",
        "Invalid unit dimension",
    ) {
        return match mapped {
            UnitDatabaseError::Conflict(message) => CreateUnitError::ConflictError(message),
            UnitDatabaseError::Validation(message) => CreateUnitError::ValidationError(message),
        };
    }

    CreateUnitError::UnexpectedError(error.into())
}
