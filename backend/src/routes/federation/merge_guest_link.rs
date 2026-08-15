use super::model::{GuestLinkResponse, guest_link_audit_details};
use super::queries::{FederationDatabaseError, fetch_guest_link, fetch_guest_link_user_for_update};
use super::security::FEDERATION_DISABLED;
use super::service::{MANAGE_FEDERATION_FORBIDDEN, merge_guest_link_user, validate_target_guest};
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::audit::{AuditAction, AuditResource, record_audit};
use crate::configuration::FederationSettings;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeGuestLinkJsonData {
    target_guest_user_id: Uuid,
}

#[derive(thiserror::Error)]
pub enum MergeGuestLinkError {
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

impl std::fmt::Debug for MergeGuestLinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for MergeGuestLinkError {
    fn status_code(&self) -> StatusCode {
        match self {
            MergeGuestLinkError::ValidationError(_) => StatusCode::BAD_REQUEST,
            MergeGuestLinkError::Forbidden(_) => StatusCode::FORBIDDEN,
            MergeGuestLinkError::NotFound(_) => StatusCode::NOT_FOUND,
            MergeGuestLinkError::ConflictError(_) => StatusCode::CONFLICT,
            MergeGuestLinkError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<FederationDatabaseError> for MergeGuestLinkError {
    fn from(error: FederationDatabaseError) -> Self {
        match error {
            FederationDatabaseError::Validation(message) => Self::ValidationError(message),
            FederationDatabaseError::Conflict(message) => Self::ConflictError(message),
            FederationDatabaseError::Unexpected(error) => Self::UnexpectedError(error),
        }
    }
}

#[tracing::instrument(
    name = "Merge federation guest link",
    skip(pool, settings, payload),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=tracing::field::Empty, link_id=tracing::field::Empty)
)]
pub async fn merge_guest_link(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_context: LaboratoryContext,
    link_id: web::Path<Uuid>,
    payload: web::Json<MergeGuestLinkJsonData>,
) -> Result<HttpResponse, MergeGuestLinkError> {
    if !settings.enabled {
        return Err(MergeGuestLinkError::Forbidden(FEDERATION_DISABLED.into()));
    }
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    let link_id = link_id.into_inner();
    tracing::Span::current().record("laboratory_id", tracing::field::display(laboratory_id));
    tracing::Span::current().record("link_id", tracing::field::display(link_id));
    let authorization_actor = laboratory_context.authorization_actor();
    if !validate_permission(
        &pool,
        &authorization_actor,
        ResourceType::Federation,
        Action::Update(laboratory_id),
    )
    .await?
    {
        return Err(MergeGuestLinkError::Forbidden(
            MANAGE_FEDERATION_FORBIDDEN.into(),
        ));
    }
    let actor = laboratory_context.actor();
    let target_guest_user_id = payload.target_guest_user_id;
    validate_target_guest(&pool, laboratory_id, target_guest_user_id).await?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    // Reading the account the link points at now locks the row, so a concurrent
    // merge cannot delete the shadow guest this one is about to leave behind.
    let old_guest_user_id =
        fetch_guest_link_user_for_update(&mut transaction, laboratory_id, link_id)
            .await?
            .ok_or_else(|| {
                MergeGuestLinkError::NotFound("Federation guest link not found".into())
            })?;
    merge_guest_link_user(
        &mut transaction,
        laboratory_id,
        link_id,
        old_guest_user_id,
        target_guest_user_id,
    )
    .await?;
    record_audit(
        &mut transaction,
        actor,
        AuditAction::Update,
        AuditResource::FederationGuestLink,
        Some(link_id),
        guest_link_audit_details(link_id, target_guest_user_id),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to merge a federation guest link")?;

    let link = fetch_guest_link(&pool, laboratory_id, link_id)
        .await?
        .ok_or_else(|| MergeGuestLinkError::NotFound("Federation guest link not found".into()))?;

    Ok(HttpResponse::Ok().json(GuestLinkResponse::from(link)))
}
