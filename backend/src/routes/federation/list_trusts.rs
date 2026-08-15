use super::queries::fetch_trusts;
use super::security::FEDERATION_DISABLED;
use super::service::READ_FEDERATION_FORBIDDEN;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::configuration::FederationSettings;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum ListTrustsError {
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListTrustsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListTrustsError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListTrustsError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListTrustsError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "List federation trusts",
    skip(pool, settings),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn list_trusts(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_context: LaboratoryContext,
) -> Result<HttpResponse, ListTrustsError> {
    if !settings.enabled {
        return Err(ListTrustsError::Forbidden(FEDERATION_DISABLED.into()));
    }
    let authorization_actor = laboratory_context.authorization_actor();
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    if !validate_permission(
        &pool,
        &authorization_actor,
        ResourceType::Federation,
        Action::Browse(laboratory_id),
    )
    .await?
    {
        return Err(ListTrustsError::Forbidden(READ_FEDERATION_FORBIDDEN.into()));
    }
    let trusts = fetch_trusts(&pool, laboratory_id).await?;

    Ok(HttpResponse::Ok().json(trusts))
}
