use super::model::{
    LaboratoryResponse, LaboratoryRow, fetch_laboratory, update_laboratory_rollback_details,
};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::UserId;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, Postgres, Transaction};
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
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(UpdateLaboratoryError::UnexpectedError)?
        .ok_or(UpdateLaboratoryError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_admin_permission(&actor)?;

    let existing =
        fetch_laboratory(&pool, *laboratory_id)
            .await?
            .ok_or(UpdateLaboratoryError::NotFound(
                "Laboratory not found".into(),
            ))?;
    validate_scoped_laboratory_permission(&actor, existing.laboratory_id)?;

    let payload = payload.into_inner();
    let name = payload
        .name
        .as_deref()
        .map(|name| required_text(name, "name"))
        .transpose()?;
    let address = payload
        .address
        .as_deref()
        .map(|address| required_text(address, "address"))
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
        &actor,
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

fn validate_admin_permission(actor: &Actor) -> Result<(), UpdateLaboratoryError> {
    if actor.is_admin() {
        Ok(())
    } else {
        Err(UpdateLaboratoryError::Forbidden(
            "You don't have permission to update laboratories.".into(),
        ))
    }
}

fn validate_scoped_laboratory_permission(
    actor: &Actor,
    laboratory_id: Uuid,
) -> Result<(), UpdateLaboratoryError> {
    if actor.is_lab_admin() && actor.laboratory_id.map(Uuid::from) != Some(laboratory_id) {
        Err(UpdateLaboratoryError::Forbidden(
            "You don't have permission to update this laboratory.".into(),
        ))
    } else if actor.is_root() || actor.is_super_admin() || actor.is_lab_admin() {
        Ok(())
    } else {
        Err(UpdateLaboratoryError::Forbidden(
            "You don't have permission to update this laboratory.".into(),
        ))
    }
}

fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, UpdateLaboratoryError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(UpdateLaboratoryError::ValidationError(format!(
            "{field} is required"
        )));
    }
    Ok(trimmed)
}

#[tracing::instrument(
    name = "Updating laboratory in the database",
    skip(transaction, name, address, description, contact),
    fields(laboratory_id=%laboratory_id)
)]
async fn update_laboratory_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: Uuid,
    name: Option<&str>,
    address: Option<&str>,
    should_update_description: bool,
    description: Option<&str>,
    should_update_contact: bool,
    contact: Option<&str>,
) -> Result<LaboratoryRow, UpdateLaboratoryError> {
    sqlx::query_as!(
        LaboratoryRow,
        r#"
        UPDATE laboratories
        SET
            name = COALESCE($2, name),
            address = COALESCE($3, address),
            description = CASE WHEN $4 THEN $5 ELSE description END,
            contact = CASE WHEN $6 THEN $7 ELSE contact END,
            updated_at = now()
        WHERE laboratory_id = $1
        RETURNING laboratory_id, name, address, description, contact, created_at, updated_at
        "#,
        laboratory_id,
        name,
        address,
        should_update_description,
        description,
        should_update_contact,
        contact,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

fn map_database_error(error: sqlx::Error) -> UpdateLaboratoryError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("laboratories_name_key")) => {
                return UpdateLaboratoryError::ConflictError(
                    "Laboratory name already exists".into(),
                );
            }
            (Some("23505"), _) => {
                return UpdateLaboratoryError::ConflictError("Laboratory already exists".into());
            }
            _ => {}
        }
    }
    UpdateLaboratoryError::UnexpectedError(error.into())
}
