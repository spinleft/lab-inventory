use super::model::GuestLinkResponse;
use super::queries::fetch_guest_links;
use super::security::FEDERATION_DISABLED;
use super::service::MANAGE_FEDERATION_FORBIDDEN;
use crate::access_control::{Action, LaboratoryContext, ResourceType, validate_permission};
use crate::configuration::FederationSettings;
use crate::utils::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum ListGuestLinksError {
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ListGuestLinksError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ListGuestLinksError {
    fn status_code(&self) -> StatusCode {
        match self {
            ListGuestLinksError::Forbidden(_) => StatusCode::FORBIDDEN,
            ListGuestLinksError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "List federation guest links",
    skip(pool, settings),
    fields(actor_user_id=%laboratory_context.actor().user_id, laboratory_id=%laboratory_context)
)]
pub async fn list_guest_links(
    pool: web::Data<PgPool>,
    settings: web::Data<FederationSettings>,
    laboratory_context: LaboratoryContext,
) -> Result<HttpResponse, ListGuestLinksError> {
    if !settings.enabled {
        return Err(ListGuestLinksError::Forbidden(FEDERATION_DISABLED.into()));
    }
    let authorization_actor = laboratory_context.authorization_actor();
    let laboratory_id = Uuid::from(laboratory_context.laboratory_id());
    if !validate_permission(
        &pool,
        &authorization_actor,
        ResourceType::Federation,
        Action::BrowseInternal(laboratory_id),
    )
    .await?
    {
        return Err(ListGuestLinksError::Forbidden(
            MANAGE_FEDERATION_FORBIDDEN.into(),
        ));
    }
    let links = fetch_guest_links(&pool, laboratory_id).await?;

    Ok(HttpResponse::Ok().json(
        links
            .into_iter()
            .map(GuestLinkResponse::from)
            .collect::<Vec<_>>(),
    ))
}
