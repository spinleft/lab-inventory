use super::model::trust_audit_details;
use super::queries::{FederationDatabaseError, revoke_trust_in_database};
use super::security::FEDERATION_DISABLED;
use super::service::MANAGE_FEDERATION_FORBIDDEN;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::configuration::FederationSettings;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum RevokeTrustError {
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

impl std::fmt::Debug for RevokeTrustError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for RevokeTrustError {
    fn status_code(&self) -> StatusCode {
        match self {
            RevokeTrustError::ValidationError(_) => StatusCode::BAD_REQUEST,
            RevokeTrustError::Forbidden(_) => StatusCode::FORBIDDEN,
            RevokeTrustError::NotFound(_) => StatusCode::NOT_FOUND,
            RevokeTrustError::ConflictError(_) => StatusCode::CONFLICT,
            RevokeTrustError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<FederationDatabaseError> for RevokeTrustError {
    fn from(error: FederationDatabaseError) -> Self {
        match error {
            FederationDatabaseError::Validation(message) => Self::ValidationError(message),
            FederationDatabaseError::Conflict(message) => Self::ConflictError(message),
            FederationDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Revoke federation trust",
    skip(pool, settings),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=tracing::field::Empty, trust_id=tracing::field::Empty)
)]
pub async fn revoke_trust(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_context: LaboratoryContext,
    trust_id: web::Path<Uuid>,
) -> Result<HttpResponse, RevokeTrustError> {
    if !settings.enabled {
        return Err(RevokeTrustError::Forbidden(FEDERATION_DISABLED.into()));
    }
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    let trust_id = trust_id.into_inner();
    tracing::Span::current().record("laboratory_id", tracing::field::display(laboratory_id));
    tracing::Span::current().record("trust_id", tracing::field::display(trust_id));
    let authorization_actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &authorization_actor,
        ResourceType::Federation,
        Action::Delete(laboratory_id),
    )
    .await?
    {
        return Err(RevokeTrustError::Forbidden(
            MANAGE_FEDERATION_FORBIDDEN.into(),
        ));
    }
    let actor = laboratory_context.actor();
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let trust = revoke_trust_in_database(&mut transaction, laboratory_id, trust_id)
        .await?
        .ok_or_else(|| RevokeTrustError::NotFound("Federation trust not found".into()))?;
    record_audit(
        &mut transaction,
        actor,
        AuditAction::Delete,
        AuditResource::FederationTrust,
        Some(trust.trust_id),
        trust_audit_details(&trust),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to revoke a federation trust")?;

    Ok(HttpResponse::NoContent().finish())
}
