use super::model::{LaboratoryResponse, LaboratoryRow, create_laboratory_rollback_details};
use crate::access_control::{Actor, get_actor};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::domain::UserId;
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
    name: String,
    address: String,
    description: Option<String>,
    contact: Option<String>,
}

#[derive(thiserror::Error)]
pub enum CreateLaboratoryError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    ConflictError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CreateLaboratoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for CreateLaboratoryError {
    fn status_code(&self) -> StatusCode {
        match self {
            CreateLaboratoryError::ValidationError(_) => StatusCode::BAD_REQUEST,
            CreateLaboratoryError::Forbidden(_) => StatusCode::FORBIDDEN,
            CreateLaboratoryError::ConflictError(_) => StatusCode::CONFLICT,
            CreateLaboratoryError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Create a laboratory",
    skip(pool, payload),
    fields(actor_user_id=%actor_user_id, laboratory_name=%payload.name)
)]
pub async fn create_laboratory(
    actor_user_id: UserId,
    pool: web::Data<PgPool>,
    payload: web::Json<JsonData>,
) -> Result<HttpResponse, CreateLaboratoryError> {
    let actor = get_actor(&pool, actor_user_id)
        .await
        .map_err(CreateLaboratoryError::UnexpectedError)?
        .ok_or(CreateLaboratoryError::Forbidden(
            "Actor not found in the database".into(),
        ))?;
    validate_create_permission(&actor)?;

    let payload = payload.into_inner();
    let name = required_text(&payload.name, "name")?;
    let address = required_text(&payload.address, "address")?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let laboratory = insert_new_laboratory(
        &mut transaction,
        name,
        address,
        payload.description.as_deref(),
        payload.contact.as_deref(),
    )
    .await?;

    record_audit(
        &mut transaction,
        &actor,
        AuditAction::Create,
        AuditResource::Laboratory,
        Some(laboratory.laboratory_id),
        create_laboratory_rollback_details(&laboratory),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new laboratory.")?;

    Ok(HttpResponse::Created().json(LaboratoryResponse::from(laboratory)))
}

fn validate_create_permission(actor: &Actor) -> Result<(), CreateLaboratoryError> {
    if actor.is_admin() {
        Ok(())
    } else {
        Err(CreateLaboratoryError::Forbidden(
            "You don't have permission to create laboratories.".into(),
        ))
    }
}

fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, CreateLaboratoryError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CreateLaboratoryError::ValidationError(format!(
            "{field} is required"
        )));
    }
    Ok(trimmed)
}

#[tracing::instrument(
    name = "Saving new laboratory in the database",
    skip(transaction, name, address, description, contact)
)]
async fn insert_new_laboratory(
    transaction: &mut Transaction<'_, Postgres>,
    name: &str,
    address: &str,
    description: Option<&str>,
    contact: Option<&str>,
) -> Result<LaboratoryRow, CreateLaboratoryError> {
    sqlx::query_as!(
        LaboratoryRow,
        r#"
        INSERT INTO laboratories (laboratory_id, name, address, description, contact)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING laboratory_id, name, address, description, contact, created_at, updated_at
        "#,
        Uuid::new_v4(),
        name,
        address,
        description,
        contact,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

fn map_database_error(error: sqlx::Error) -> CreateLaboratoryError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("laboratories_name_key")) => {
                return CreateLaboratoryError::ConflictError(
                    "Laboratory name already exists".into(),
                );
            }
            (Some("23505"), _) => {
                return CreateLaboratoryError::ConflictError("Laboratory already exists".into());
            }
            _ => {}
        }
    }
    CreateLaboratoryError::UnexpectedError(error.into())
}
