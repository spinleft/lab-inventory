use super::model::{
    LocationResponse, LocationRow, create_location_rollback_details, fetch_location_for_update,
    map_database_conflict,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::{LaboratoryId, UserId};
use crate::domain::{LocationCode, LocationId, LocationName, NewLocation};
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
    parent_location_id: Option<Uuid>,
    name: String,
    code: String,
    description: Option<String>,
}

impl TryFrom<JsonData> for NewLocation {
    type Error = String;

    fn try_from(value: JsonData) -> Result<Self, Self::Error> {
        let parent_location_id = value
            .parent_location_id
            .map(LocationId::parse)
            .transpose()?;
        let name = LocationName::parse(value.name)?;
        let code = LocationCode::parse(value.code)?;

        Ok(Self::new(parent_location_id, name, code, value.description))
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

#[tracing::instrument(
    name = "Create a location",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn create_location(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateLocationError> {
    let laboratory_id = LaboratoryId::parse(laboratory_id.into_inner())
        .map_err(CreateLocationError::ValidationError)?;
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(CreateLocationError::UnexpectedError)?
        .ok_or(CreateLocationError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    let new_location = NewLocation::try_from(payload.into_inner())
        .map_err(CreateLocationError::ValidationError)?;
    validate_create_permission(&actor, &laboratory_id)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let parent = fetch_parent_location(&mut transaction, new_location.parent_location_id).await?;
    validate_parent(&parent, &laboratory_id)?;
    let parent_location_id = new_location.parent_location_id;
    let (path, depth) = build_path_and_depth(parent.as_ref(), new_location.code.as_ref());
    let location = insert_location(
        &mut transaction,
        laboratory_id,
        parent_location_id,
        new_location.name.as_ref(),
        new_location.code.as_ref(),
        &path,
        depth,
        new_location.description.as_deref(),
    )
    .await?;

    record_audit(
        &mut transaction,
        &actor,
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

fn validate_create_permission(
    actor: &Actor,
    target_laboratory_id: &LaboratoryId,
) -> Result<(), CreateLocationError> {
    if actor.can_write_laboratory_resource(target_laboratory_id) {
        Ok(())
    } else {
        Err(CreateLocationError::Forbidden(
            "You don't have permission to create locations for this laboratory.".into(),
        ))
    }
}

async fn fetch_parent_location(
    transaction: &mut Transaction<'_, Postgres>,
    parent_location_id: Option<LocationId>,
) -> Result<Option<LocationRow>, CreateLocationError> {
    let Some(parent_location_id) = parent_location_id else {
        return Ok(None);
    };

    fetch_location_for_update(transaction, parent_location_id)
        .await?
        .ok_or(CreateLocationError::ValidationError(
            "Parent location not found".into(),
        ))
        .map(Some)
}

fn validate_parent(
    parent: &Option<LocationRow>,
    laboratory_id: &LaboratoryId,
) -> Result<(), CreateLocationError> {
    if let Some(parent) = parent {
        if &LaboratoryId::parse(parent.laboratory_id)
            .map_err(CreateLocationError::ValidationError)?
            != laboratory_id
        {
            return Err(CreateLocationError::ValidationError(
                "Parent location does not belong to this laboratory".into(),
            ));
        }
    }
    Ok(())
}

fn build_path_and_depth(parent: Option<&LocationRow>, code: &str) -> (String, i32) {
    match parent {
        Some(parent) => (format!("{}.{}", parent.path, code), parent.depth + 1),
        None => (code.to_string(), 0),
    }
}

#[tracing::instrument(
    name = "Saving new location in the database",
    skip(transaction, name, code, path, description),
    fields(laboratory_id=%laboratory_id)
)]
async fn insert_location(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    parent_location_id: Option<LocationId>,
    name: &str,
    code: &str,
    path: &str,
    depth: i32,
    description: Option<&str>,
) -> Result<LocationRow, CreateLocationError> {
    sqlx::query_as!(
        LocationRow,
        r#"
        INSERT INTO locations (
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path,
            depth,
            description
        )
        VALUES ($1, $2, $3, $4, $5, $6::text::ltree, $7, $8)
        RETURNING
            location_id,
            laboratory_id,
            parent_location_id,
            name,
            code,
            path::text AS "path!",
            depth,
            description,
            created_at,
            updated_at
        "#,
        Uuid::new_v4(),
        *laboratory_id,
        parent_location_id.map(Uuid::from),
        name,
        code,
        path,
        depth,
        description,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

fn map_database_error(error: sqlx::Error) -> CreateLocationError {
    if let Some(message) = map_database_conflict(
        &error,
        "Location name already exists under this parent",
        "Location code already exists under this parent",
        "Location path already exists",
        "Location already exists",
    ) {
        return CreateLocationError::ConflictError(message);
    }

    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23503") {
            return CreateLocationError::ValidationError("Invalid laboratory".into());
        }
    }

    CreateLocationError::UnexpectedError(error.into())
}
