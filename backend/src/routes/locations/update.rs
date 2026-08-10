use super::model::{LocationResponse, update_location_rollback_details};
use super::queries::{LocationDatabaseError, fetch_location_for_update};
use super::service::{build_path_and_depth, move_location, resolve_moved_parent};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{
    LocationCode, LocationId, LocationName, NullableUpdate, UpdateLocation, UserId,
};
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    #[serde(default, deserialize_with = "deserialize_nullable")]
    parent_location_id: Option<Option<Uuid>>,
    name: Option<String>,
    code: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    description: Option<Option<String>>,
}

impl TryFrom<JsonData> for UpdateLocation {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let parent_location_id = value.parent_location_id.map(|id| id.map(Uuid::into)).into();
        let name = value.name.map(LocationName::parse).transpose()?;
        let code = value.code.map(LocationCode::parse).transpose()?;
        let description = match value.description {
            Some(Some(description)) => NullableUpdate::Set(description),
            Some(None) => NullableUpdate::Clear,
            None => NullableUpdate::Unchanged,
        };

        Ok(Self {
            parent_location_id,
            name,
            code,
            description,
        })
    }
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(thiserror::Error)]
pub enum UpdateLocationError {
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

impl std::fmt::Debug for UpdateLocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UpdateLocationError {
    fn status_code(&self) -> StatusCode {
        match self {
            UpdateLocationError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UpdateLocationError::Forbidden(_) => StatusCode::FORBIDDEN,
            UpdateLocationError::NotFound(_) => StatusCode::NOT_FOUND,
            UpdateLocationError::ConflictError(_) => StatusCode::CONFLICT,
            UpdateLocationError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<LocationDatabaseError> for UpdateLocationError {
    fn from(error: LocationDatabaseError) -> Self {
        match error {
            LocationDatabaseError::Validation(message) => Self::ValidationError(message),
            LocationDatabaseError::Conflict(message) => Self::ConflictError(message),
            LocationDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Update a location",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, location_id=%location_id)
)]
pub async fn update_location(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    location_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateLocationError> {
    let location_id: LocationId = location_id.into_inner().into();
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::Location,
        Action::Update(location_id.into()),
    )
    .await?
    {
        return Err(UpdateLocationError::Forbidden(
            "You don't have permission to update this location.".into(),
        ));
    }

    let update_location = UpdateLocation::try_from(payload.into_inner())
        .map_err(UpdateLocationError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let existing = fetch_location_for_update(&mut transaction, location_id)
        .await?
        .ok_or(UpdateLocationError::NotFound("Location not found".into()))?;

    let name = update_location
        .name
        .as_ref()
        .map(|name| name.as_ref())
        .unwrap_or(&existing.name)
        .to_string();
    let code = update_location
        .code
        .as_ref()
        .map(|code| code.as_ref())
        .unwrap_or(&existing.code)
        .to_string();
    let current_parent_location_id = existing.parent_location_id.map(Uuid::into);
    let parent_location_id = update_location
        .parent_location_id
        .resolve(current_parent_location_id);
    let description = update_location
        .description
        .resolve(existing.description.clone());

    let parent = resolve_moved_parent(&mut transaction, &existing, parent_location_id).await?;
    let (path, depth) = build_path_and_depth(parent.as_ref(), &code);
    let updated = move_location(
        &mut transaction,
        &existing,
        parent_location_id,
        &name,
        &code,
        &path,
        depth,
        description.as_deref(),
    )
    .await?;

    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Update,
        AuditResource::Location,
        Some(updated.location_id),
        update_location_rollback_details(&existing),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update a location.")?;

    Ok(HttpResponse::Ok().json(LocationResponse::from(updated)))
}
