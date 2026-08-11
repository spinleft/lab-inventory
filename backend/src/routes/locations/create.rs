use super::model::{LocationResponse, create_location_rollback_details};
use super::queries::{LocationDatabaseError, insert_location};
use super::service::{build_path_and_depth, resolve_new_parent};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{LocationCode, LocationName, NewLocation};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    parent_location_id: Option<Uuid>,
    name: String,
    code: String,
    description: Option<String>,
}

impl TryFrom<JsonData> for NewLocation {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let parent_location_id = value.parent_location_id.map(Uuid::into);
        let name = LocationName::parse(value.name)?;
        let code = LocationCode::parse(value.code)?;

        Ok(Self {
            parent_location_id,
            name,
            code,
            description: value.description,
        })
    }
}

#[derive(thiserror::Error)]
pub enum CreateLocationError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreateLocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateLocationError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateLocationError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateLocationError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateLocationError::ConflictError(_) => StatusCode::CONFLICT,
            CreateLocationError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<LocationDatabaseError> for CreateLocationError {
    fn from(error: LocationDatabaseError) -> Self {
        match error {
            LocationDatabaseError::Validation(message) => Self::ValidationError(message),
            LocationDatabaseError::Conflict(message) => Self::ConflictError(message),
            LocationDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Create a location",
    skip(pool, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn create_location(
    pool: web::Data<PgPool>,
    laboratory_context: LaboratoryContext,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateLocationError> {
    let actor = laboratory_context.actor();
    let laboratory_id = laboratory_context.laboratory_id();
    if !validate_permission(
        &pool,
        actor,
        ResourceType::Location,
        Action::Create(laboratory_id.into()),
    )
    .await?
    {
        return Err(CreateLocationError::Forbidden(
            "You don't have permission to create locations.".into(),
        ));
    }

    let new_location = NewLocation::try_from(payload.into_inner())
        .map_err(CreateLocationError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let parent = resolve_new_parent(
        &mut transaction,
        laboratory_id,
        new_location.parent_location_id,
    )
    .await?;
    let (path, depth) = build_path_and_depth(parent.as_ref(), new_location.code.as_ref());
    let location = insert_location(
        &mut transaction,
        laboratory_id,
        new_location.parent_location_id,
        new_location.name.as_ref(),
        new_location.code.as_ref(),
        &path,
        depth,
        new_location.description.as_deref(),
    )
    .await?;

    record_audit(
        &mut transaction,
        actor,
        AuditAction::Create,
        AuditResource::Location,
        Some(location.location_id),
        create_location_rollback_details(&location),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new location.")?;

    Ok(HttpResponse::Created().json(LocationResponse::from(location)))
}
