use super::model::{LaboratoryResponse, update_laboratory_rollback_details};
use super::queries::{
    LaboratoryDatabaseError, fetch_laboratory, update_laboratory_in_database,
};
use crate::access_control::{Action, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::UserId;
use crate::utils::{error_chain_fmt, required_text};
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonData {
    name: Option<String>,
    address: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    contact: Option<Option<String>>,
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(thiserror::Error)]
pub enum UpdateLaboratoryError {
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

impl std::fmt::Debug for UpdateLaboratoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for UpdateLaboratoryError {
    fn status_code(&self) -> StatusCode {
        match self {
            UpdateLaboratoryError::ValidationError(_) => StatusCode::BAD_REQUEST,
            UpdateLaboratoryError::Forbidden(_) => StatusCode::FORBIDDEN,
            UpdateLaboratoryError::NotFound(_) => StatusCode::NOT_FOUND,
            UpdateLaboratoryError::ConflictError(_) => StatusCode::CONFLICT,
            UpdateLaboratoryError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<LaboratoryDatabaseError> for UpdateLaboratoryError {
    fn from(error: LaboratoryDatabaseError) -> Self {
        match error {
            LaboratoryDatabaseError::Conflict(message) => Self::ConflictError(message),
            LaboratoryDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Update a laboratory",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, laboratory_id=%laboratory_id)
)]
pub async fn update_laboratory(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    laboratory_id: web::Path<Uuid>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, UpdateLaboratoryError> {
    let existing =
        fetch_laboratory(&pool, *laboratory_id)
            .await?
            .ok_or(UpdateLaboratoryError::NotFound(
                "Laboratory not found".into(),
            ))?;
    if !validate_permission(
        &pool,
        &actor_user_id,
        ResourceType::Laboratory,
        Action::Update(existing.laboratory_id),
    )
    .await?
    {
        return Err(UpdateLaboratoryError::Forbidden(
            "You don't have permission to update this laboratory.".into(),
        ));
    }

    let payload = payload.into_inner();
    let name = payload
        .name
        .as_deref()
        .map(|name| required_text(name, "name").map_err(UpdateLaboratoryError::ValidationError))
        .transpose()?;
    let address = payload
        .address
        .as_deref()
        .map(|address| {
            required_text(address, "address").map_err(UpdateLaboratoryError::ValidationError)
        })
        .transpose()?;
    let should_update_description = payload.description.is_some();
    let description = payload
        .description
        .as_ref()
        .and_then(|value| value.as_deref());
    let should_update_contact = payload.contact.is_some();
    let contact = payload.contact.as_ref().and_then(|value| value.as_deref());

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let laboratory = update_laboratory_in_database(
        &mut transaction,
        existing.laboratory_id,
        name,
        address,
        should_update_description,
        description,
        should_update_contact,
        contact,
    )
    .await?;

    record_audit(
        &mut transaction,
        actor_user_id,
        AuditAction::Update,
        AuditResource::Laboratory,
        Some(laboratory.laboratory_id),
        update_laboratory_rollback_details(&existing),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to update a laboratory.")?;

    Ok(HttpResponse::Ok().json(LaboratoryResponse::from(laboratory)))
}
