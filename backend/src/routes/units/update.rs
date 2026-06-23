use super::model::{
    UnitDatabaseError, UnitResponse, UnitRow, fetch_unit_for_update, map_unit_database_error,
    update_unit_rollback_details,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{UnitCode, UnitDimension, UnitName, UnitSymbol, UpdateUnit, UserId};
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
    code: Option<String>,
    name: Option<String>,
    symbol: Option<String>,
    dimension: Option<String>,
    scale_to_base: Option<f64>,
    allow_decimal: Option<bool>,
}

impl TryFrom<JsonData> for UpdateUnit {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        Self::new(
            value.code.map(UnitCode::parse).transpose()?,
            value.name.map(UnitName::parse).transpose()?,
            value.symbol.map(UnitSymbol::parse).transpose()?,
            value
                .dimension
                .as_deref()
                .map(UnitDimension::parse)
                .transpose()?,
            value.scale_to_base,
            value.allow_decimal,
        )
    }
}

#[derive(thiserror::Error)]
pub enum UpdateUnitError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for UpdateUnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UpdateUnitError {
    fn status_code(&self) -> StatusCode {
        match self {
            UpdateUnitError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UpdateUnitError::Forbidden(_) => StatusCode::FORBIDDEN,
            UpdateUnitError::NotFound(_) => StatusCode::NOT_FOUND,
            UpdateUnitError::ConflictError(_) => StatusCode::CONFLICT,
            UpdateUnitError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Update a unit",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, unit_id=%unit_id)
)]
pub async fn update_unit(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    unit_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateUnitError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(UpdateUnitError::UnexpectedError)?
        .ok_or(UpdateUnitError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_update_permission(&actor)?;
    let update_unit =
        UpdateUnit::try_from(payload.into_inner()).map_err(UpdateUnitError::ValidationError)?;
    let dimension = update_unit.dimension.map(|dimension| dimension.to_string());

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_unit_for_update(&mut transaction, *unit_id)
        .await?
        .ok_or(UpdateUnitError::NotFound("Unit not found".into()))?;
    let unit = update_unit_in_database(
        &mut transaction,
        existing.unit_id,
        update_unit.code.as_ref().map(|code| code.as_ref()),
        update_unit.name.as_ref().map(|name| name.as_ref()),
        update_unit.symbol.as_ref().map(|symbol| symbol.as_ref()),
        dimension.as_deref(),
        update_unit.scale_to_base,
        update_unit.allow_decimal,
    )
    .await?;

    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Update,
        AuditResource::Unit,
        Some(unit.unit_id),
        update_unit_rollback_details(&existing),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update a unit.")?;

    Ok(HttpResponse::Ok().json(UnitResponse::from(unit)))
}

fn validate_update_permission(actor: &Actor) -> Result<(), UpdateUnitError> {
    if actor.can_manage_units() {
        Ok(())
    } else {
        Err(UpdateUnitError::Forbidden(
            "You don't have permission to update units.".into(),
        ))
    }
}

#[tracing::instrument(
    name = "Updating unit in the database",
    skip(transaction, code, name, symbol, dimension),
    fields(unit_id=%unit_id)
)]
async fn update_unit_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
    code: Option<&str>,
    name: Option<&str>,
    symbol: Option<&str>,
    dimension: Option<&str>,
    scale_to_base: Option<f64>,
    allow_decimal: Option<bool>,
) -> Result<UnitRow, UpdateUnitError> {
    sqlx::query_as!(
        UnitRow,
        r#"
        UPDATE units
        SET
            code = COALESCE($2, code),
            name = COALESCE($3, name),
            symbol = COALESCE($4, symbol),
            dimension = COALESCE($5, dimension),
            scale_to_base = COALESCE($6, scale_to_base),
            allow_decimal = COALESCE($7, allow_decimal)
        WHERE unit_id = $1
        RETURNING unit_id, code, name, symbol, dimension, scale_to_base, allow_decimal, created_at
        "#,
        unit_id,
        code,
        name,
        symbol,
        dimension,
        scale_to_base,
        allow_decimal,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

fn map_database_error(error: sqlx::Error) -> UpdateUnitError {
    if let Some(mapped) = map_unit_database_error(
        &error,
        "Unit code already exists",
        "Unit already exists",
        "Invalid unit",
        "Invalid unit dimension",
    ) {
        return match mapped {
            UnitDatabaseError::Conflict(message) => UpdateUnitError::ConflictError(message),
            UnitDatabaseError::Validation(message) => UpdateUnitError::ValidationError(message),
        };
    }

    UpdateUnitError::UnexpectedError(error.into())
}
